//! ONNX inference path for `DistilBartSummarizer`.
//!
//! Split into a sibling submodule because the encoder + decoder loop is
//! ~300 lines of careful tensor wiring and the parent file already
//! carries the type, builder, trait impl, and ~80 lines of tests.

use super::{DistilBartSummarizer, InferenceState};
use ndarray::{Array2, Array3, Array4};
use ort::session::Session;
use ort::tensor::Shape;
use ort::value::Value;
use std::sync::Arc;
use tokio::sync::OnceCell;

impl DistilBartSummarizer {
    pub(super) async fn summarize_inner(&self, text: &str) -> anyhow::Result<String> {
        let state = self.ensure_state().await?;
        let text_owned = text.to_string();
        let max_input = self.max_input_length;
        let max_output = self.max_output_length;
        let min_output = self.min_output_length;
        let decoder_start = self.decoder_start_token_id;
        let eos = self.eos_token_id;
        let pad = self.pad_token_id;
        let _ = pad; // pad_token_id is reserved for batched extensions.

        let state_for_blocking = Arc::clone(&state);
        tokio::task::spawn_blocking(move || {
            run_generation(
                &state_for_blocking,
                &text_owned,
                max_input,
                max_output,
                min_output,
                decoder_start,
                eos,
            )
        })
        .await
        .map_err(|e| anyhow::anyhow!("DistilBartSummarizer task panicked: {e}"))?
    }

    pub(super) async fn ensure_state(&self) -> anyhow::Result<Arc<InferenceState>> {
        let encoder_path = self.encoder_path.clone();
        let decoder_path = self.decoder_path.clone();
        let decoder_with_past_path = self.decoder_with_past_path.clone();
        let tokenizer_path = self.tokenizer_path.clone();
        let cell: &OnceCell<Arc<InferenceState>> = &self.state;
        let arc = cell
            .get_or_try_init(|| async move {
                tokio::task::spawn_blocking(move || {
                    load_state(
                        &encoder_path,
                        &decoder_path,
                        &decoder_with_past_path,
                        &tokenizer_path,
                    )
                })
                .await
                .map_err(|e| anyhow::anyhow!("DistilBartSummarizer init task panicked: {e}"))?
                .map(Arc::new)
            })
            .await?;
        Ok(Arc::clone(arc))
    }
}

fn load_state(
    encoder_path: &std::path::PathBuf,
    decoder_path: &std::path::PathBuf,
    decoder_with_past_path: &std::path::PathBuf,
    tokenizer_path: &std::path::PathBuf,
) -> anyhow::Result<InferenceState> {
    use anyhow::Context;

    let mut tokenizer = tokenizers::Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow::anyhow!("failed to load tokenizer from {tokenizer_path:?}: {e}"))?;
    // BART can summarize prompts longer than `max_position_embeddings` only
    // by truncation at the encoder boundary. We do that ourselves below
    // (`encoding.truncate(max_input_length)`), so disable any tokenizer-
    // embedded truncation policy here to avoid double truncation surprises.
    tokenizer
        .with_truncation(None)
        .map_err(|e| anyhow::anyhow!("failed to disable tokenizer truncation: {e}"))?;

    let encoder = Session::builder()
        .context("failed to create ort SessionBuilder for encoder")?
        .commit_from_file(encoder_path)
        .with_context(|| format!("failed to load encoder ONNX from {encoder_path:?}"))?;
    let decoder = Session::builder()
        .context("failed to create ort SessionBuilder for decoder")?
        .commit_from_file(decoder_path)
        .with_context(|| format!("failed to load decoder ONNX from {decoder_path:?}"))?;
    let decoder_with_past = Session::builder()
        .context("failed to create ort SessionBuilder for decoder_with_past")?
        .commit_from_file(decoder_with_past_path)
        .with_context(|| {
            format!("failed to load decoder_with_past ONNX from {decoder_with_past_path:?}")
        })?;

    // Discover past_key_values input slots. Optimum names them
    // `past_key_values.{i}.{decoder|encoder}.{key|value}`. Discovering
    // them here lets us support different layer counts (BART-large-CNN
    // = 12 layers, distilbart-cnn-6-6 = 6 decoder layers, etc.)
    // without recompiling.
    let past_input_names: Vec<String> = decoder_with_past
        .inputs
        .iter()
        .map(|i| i.name.clone())
        .filter(|n| n.starts_with("past_key_values."))
        .collect();
    if past_input_names.is_empty() {
        anyhow::bail!(
            "decoder_with_past graph has no `past_key_values.*` inputs — \
             was the model exported with `--task text2text-generation-with-past`?"
        );
    }

    // Match each past input to its corresponding present output. Optimum
    // names presents `present.{i}.{decoder|encoder}.{key|value}` with a
    // 1:1 mapping to past inputs.
    let mut present_output_names = Vec::with_capacity(past_input_names.len());
    for past_name in &past_input_names {
        let suffix = past_name
            .strip_prefix("past_key_values.")
            .ok_or_else(|| anyhow::anyhow!("unexpected past name format: {past_name}"))?;
        let want = format!("present.{suffix}");
        // Both the no-past and with-past graphs share the same present.*
        // output names (because they're the same conceptual outputs).
        // Pick from the no-past graph since we use it on step 1.
        let exists = decoder.outputs.iter().any(|o| o.name == want);
        if !exists {
            anyhow::bail!(
                "decoder graph missing expected output `{want}` (paired with input `{past_name}`)"
            );
        }
        present_output_names.push(want);
    }

    let decoder_uses_encoder_attention_mask = decoder
        .inputs
        .iter()
        .any(|i| i.name == "encoder_attention_mask");

    Ok(InferenceState {
        encoder: std::sync::Mutex::new(encoder),
        decoder: std::sync::Mutex::new(decoder),
        decoder_with_past: std::sync::Mutex::new(decoder_with_past),
        tokenizer,
        past_input_names,
        present_output_names,
        decoder_uses_encoder_attention_mask,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_generation(
    state: &InferenceState,
    text: &str,
    max_input_length: usize,
    max_output_length: usize,
    min_output_length: usize,
    decoder_start_token_id: i64,
    eos_token_id: i64,
) -> anyhow::Result<String> {
    // ---- 1. Tokenize input + run encoder ----
    let encoding = state
        .tokenizer
        .encode(text, true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode failed: {e}"))?;
    let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
    let mut attention_mask: Vec<i64> = encoding
        .get_attention_mask()
        .iter()
        .map(|&x| x as i64)
        .collect();
    if input_ids.len() > max_input_length {
        input_ids.truncate(max_input_length);
        attention_mask.truncate(max_input_length);
    }
    let src_len = input_ids.len();
    if src_len == 0 {
        return Ok(String::new());
    }

    let encoder_input_ids = Array2::<i64>::from_shape_vec((1, src_len), input_ids.clone())?;
    let encoder_attn_mask = Array2::<i64>::from_shape_vec((1, src_len), attention_mask.clone())?;

    // Run encoder once. Owned f32 vec + shape captured so we can re-feed
    // the encoder hidden states across all decoder steps without holding
    // a borrow on the encoder's outputs (which would prevent dropping the
    // session lock).
    let (encoder_hidden_state_data, encoder_hidden_state_shape) = {
        let mut session = state
            .encoder
            .lock()
            .map_err(|e| anyhow::anyhow!("encoder mutex poisoned: {e}"))?;
        let outputs = session.run(ort::inputs![
            "input_ids" => Value::from_array(encoder_input_ids)?,
            "attention_mask" => Value::from_array(encoder_attn_mask)?,
        ])?;
        let (_name, output) = outputs
            .iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("encoder produced no outputs"))?;
        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("failed to extract encoder f32 tensor: {e}"))?;
        let dims: Vec<usize> = shape.as_ref().iter().map(|&d| d as usize).collect();
        if dims.len() != 3 || dims[0] != 1 || dims[1] != src_len {
            anyhow::bail!(
                "encoder output shape {:?} does not match expected [1, {}, d_model]",
                dims,
                src_len
            );
        }
        (data.to_vec(), dims)
    };

    let d_model = encoder_hidden_state_shape[2];
    // Reconstruct the encoder hidden states as an ndarray view-friendly
    // owned Array3 so the same buffer can be wrapped into ort::Value
    // via `from_array(.clone())` for each decoder step.
    let encoder_hidden_states =
        Array3::<f32>::from_shape_vec((1, src_len, d_model), encoder_hidden_state_data)?;

    // ---- 2. Decoder generation loop ----
    let mut generated: Vec<i64> = Vec::with_capacity(max_output_length);
    // First decoder step input is the BART decoder-start token (`</s>` for
    // CNN/DM-tuned BART, id=2).
    let mut next_input_token = decoder_start_token_id;

    // KV cache tensors for the next step's `past_key_values.*`. Layout
    // matches `state.past_input_names` 1:1.
    let mut past_key_values: Vec<KvTensor> = Vec::new();

    for step in 0..max_output_length {
        // Build decoder inputs: input_ids = [next_input_token],
        // encoder_hidden_states (cached), encoder_attention_mask (cached).
        let dec_input_ids = Array2::<i64>::from_shape_vec((1, 1), vec![next_input_token])?;
        let enc_hidden_for_step = encoder_hidden_states.clone();
        let enc_attn_mask_opt = if state.decoder_uses_encoder_attention_mask {
            Some(Array2::<i64>::from_shape_vec(
                (1, src_len),
                attention_mask.clone(),
            )?)
        } else {
            None
        };

        // Run decoder. step=0 uses the no-past graph; later steps use
        // decoder_with_past with the `past_key_values.*` from the prior
        // step's `present.*` outputs.
        let (logits_last_step, new_past) = if step == 0 {
            run_decoder_first_step(state, dec_input_ids, enc_hidden_for_step, enc_attn_mask_opt)?
        } else {
            run_decoder_with_past(
                state,
                dec_input_ids,
                enc_hidden_for_step,
                enc_attn_mask_opt,
                &past_key_values,
            )?
        };
        past_key_values = new_past;

        // Greedy: argmax over the vocab for the last (only) generated step.
        let next_token = argmax_last_step(&logits_last_step)?;

        // Force minimum length: suppress EOS until min_output_length tokens
        // have been generated. BART tends to emit short summaries on
        // short inputs without this floor.
        if next_token == eos_token_id && generated.len() < min_output_length {
            // Re-pick the next-best non-EOS token. For simplicity walk the
            // logits once more.
            let next_token = second_argmax_last_step(&logits_last_step, eos_token_id)?;
            if next_token == eos_token_id {
                break;
            }
            generated.push(next_token);
            next_input_token = next_token;
            continue;
        }

        if next_token == eos_token_id {
            break;
        }
        generated.push(next_token);
        next_input_token = next_token;
    }

    // ---- 3. Detokenize ----
    let token_ids_u32: Vec<u32> = generated.iter().map(|&t| t as u32).collect();
    if token_ids_u32.is_empty() {
        return Ok(String::new());
    }
    let decoded = state
        .tokenizer
        .decode(&token_ids_u32, true)
        .map_err(|e| anyhow::anyhow!("tokenizer decode failed: {e}"))?;
    Ok(decoded.trim().to_string())
}

/// Container for one slot in the KV cache. Owns a flat `Vec<f32>` plus the
/// 4D shape so we can rebuild an `ort::Value` per step without holding a
/// borrow on the previous step's session outputs.
struct KvTensor {
    data: Vec<f32>,
    shape: [usize; 4],
}

#[derive(Debug)]
struct DecoderLogits {
    /// Flat `[1, tgt_len, vocab]` row-major. We only ever look at the last
    /// timestep, so storing flat avoids ndarray allocation.
    flat: Vec<f32>,
    tgt_len: usize,
    vocab: usize,
}

fn run_decoder_first_step(
    state: &InferenceState,
    input_ids: Array2<i64>,
    encoder_hidden_states: Array3<f32>,
    encoder_attention_mask: Option<Array2<i64>>,
) -> anyhow::Result<(DecoderLogits, Vec<KvTensor>)> {
    let mut session = state
        .decoder
        .lock()
        .map_err(|e| anyhow::anyhow!("decoder mutex poisoned: {e}"))?;
    let mut inputs: Vec<(
        std::borrow::Cow<'_, str>,
        ort::session::SessionInputValue<'_>,
    )> = Vec::new();
    inputs.push(("input_ids".into(), Value::from_array(input_ids)?.into()));
    inputs.push((
        "encoder_hidden_states".into(),
        Value::from_array(encoder_hidden_states)?.into(),
    ));
    if let Some(mask) = encoder_attention_mask {
        inputs.push((
            "encoder_attention_mask".into(),
            Value::from_array(mask)?.into(),
        ));
    }
    let outputs = session.run(inputs)?;
    extract_logits_and_presents(state, &outputs)
}

fn run_decoder_with_past(
    state: &InferenceState,
    input_ids: Array2<i64>,
    encoder_hidden_states: Array3<f32>,
    encoder_attention_mask: Option<Array2<i64>>,
    past_key_values: &[KvTensor],
) -> anyhow::Result<(DecoderLogits, Vec<KvTensor>)> {
    if past_key_values.len() != state.past_input_names.len() {
        anyhow::bail!(
            "past_key_values length ({}) does not match expected slot count ({})",
            past_key_values.len(),
            state.past_input_names.len()
        );
    }
    let mut session = state
        .decoder_with_past
        .lock()
        .map_err(|e| anyhow::anyhow!("decoder_with_past mutex poisoned: {e}"))?;

    let mut inputs: Vec<(
        std::borrow::Cow<'_, str>,
        ort::session::SessionInputValue<'_>,
    )> = Vec::new();
    inputs.push(("input_ids".into(), Value::from_array(input_ids)?.into()));
    inputs.push((
        "encoder_hidden_states".into(),
        Value::from_array(encoder_hidden_states)?.into(),
    ));
    if let Some(mask) = encoder_attention_mask {
        inputs.push((
            "encoder_attention_mask".into(),
            Value::from_array(mask)?.into(),
        ));
    }
    for (slot_name, kv) in state.past_input_names.iter().zip(past_key_values.iter()) {
        let array = Array4::<f32>::from_shape_vec(kv.shape, kv.data.clone())?;
        inputs.push((slot_name.clone().into(), Value::from_array(array)?.into()));
    }
    let outputs = session.run(inputs)?;
    extract_logits_and_presents(state, &outputs)
}

fn extract_logits_and_presents(
    state: &InferenceState,
    outputs: &ort::session::SessionOutputs<'_>,
) -> anyhow::Result<(DecoderLogits, Vec<KvTensor>)> {
    // Pull `logits` (always present in every decoder graph variant Optimum
    // emits for summarization) and the `present.*` outputs in the order
    // that pairs 1:1 with `state.past_input_names`.
    let logits_view = outputs
        .iter()
        .find(|(name, _)| *name == "logits")
        .ok_or_else(|| anyhow::anyhow!("decoder output missing `logits`"))?
        .1;
    let (logits_shape, logits_data) = logits_view
        .try_extract_tensor::<f32>()
        .map_err(|e| anyhow::anyhow!("failed to extract logits f32 tensor: {e}"))?;
    let dims = logits_shape.as_ref();
    if dims.len() != 3 || dims[0] != 1 {
        anyhow::bail!("decoder logits shape {:?} not [1, tgt_len, vocab]", dims);
    }
    let tgt_len = dims[1] as usize;
    let vocab = dims[2] as usize;
    let logits = DecoderLogits {
        flat: logits_data.to_vec(),
        tgt_len,
        vocab,
    };

    let mut presents: Vec<KvTensor> = Vec::with_capacity(state.present_output_names.len());
    for name in &state.present_output_names {
        let value = outputs
            .iter()
            .find(|(n, _)| *n == name.as_str())
            .ok_or_else(|| anyhow::anyhow!("decoder output missing `{name}`"))?
            .1;
        let (shape, data) = value
            .try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("failed to extract `{name}` f32 tensor: {e}"))?;
        let dims = shape_to_4d(shape, name)?;
        presents.push(KvTensor {
            data: data.to_vec(),
            shape: dims,
        });
    }
    Ok((logits, presents))
}

fn shape_to_4d(shape: &Shape, name: &str) -> anyhow::Result<[usize; 4]> {
    let dims = shape.as_ref();
    if dims.len() != 4 {
        anyhow::bail!(
            "expected rank-4 KV tensor for `{name}`, got shape {:?}",
            dims
        );
    }
    Ok([
        dims[0] as usize,
        dims[1] as usize,
        dims[2] as usize,
        dims[3] as usize,
    ])
}

fn argmax_last_step(logits: &DecoderLogits) -> anyhow::Result<i64> {
    if logits.tgt_len == 0 || logits.vocab == 0 {
        anyhow::bail!("logits tensor empty");
    }
    let last_step_offset = (logits.tgt_len - 1) * logits.vocab;
    let slice = &logits.flat[last_step_offset..last_step_offset + logits.vocab];
    let (idx, _) = slice
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .ok_or_else(|| anyhow::anyhow!("argmax over empty slice"))?;
    Ok(idx as i64)
}

fn second_argmax_last_step(logits: &DecoderLogits, excluded_id: i64) -> anyhow::Result<i64> {
    if logits.tgt_len == 0 || logits.vocab == 0 {
        anyhow::bail!("logits tensor empty");
    }
    let last_step_offset = (logits.tgt_len - 1) * logits.vocab;
    let slice = &logits.flat[last_step_offset..last_step_offset + logits.vocab];
    let mut best: (i64, f32) = (-1, f32::NEG_INFINITY);
    for (i, &v) in slice.iter().enumerate() {
        if i as i64 == excluded_id {
            continue;
        }
        if v > best.1 {
            best = (i as i64, v);
        }
    }
    if best.0 < 0 {
        anyhow::bail!("argmax over empty slice (after exclusion)");
    }
    Ok(best.0)
}
