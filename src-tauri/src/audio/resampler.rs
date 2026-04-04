use rubato::{FftFixedInOut, Resampler};

/// Resample f32 mono audio from `from_rate` to `to_rate`.
/// Returns the resampled buffer.
pub fn resample_mono(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return input.to_vec();
    }

    let chunk_size = 1024;
    let mut resampler =
        FftFixedInOut::<f32>::new(from_rate as usize, to_rate as usize, chunk_size, 1)
            .expect("failed to create resampler");

    let frames_needed = resampler.input_frames_next();
    let mut output = Vec::with_capacity(input.len() * to_rate as usize / from_rate as usize + 1024);
    let mut pos = 0;

    while pos + frames_needed <= input.len() {
        let chunk = &input[pos..pos + frames_needed];
        let result = resampler.process(&[chunk], None).expect("resample failed");
        output.extend_from_slice(&result[0]);
        pos += frames_needed;
    }

    // Handle remaining samples by zero-padding
    if pos < input.len() {
        let mut last_chunk = vec![0.0f32; frames_needed];
        let remaining = input.len() - pos;
        last_chunk[..remaining].copy_from_slice(&input[pos..]);
        let result = resampler
            .process(&[&last_chunk], None)
            .expect("resample failed");
        // Only take the proportional amount of output
        let expected_out = remaining * to_rate as usize / from_rate as usize;
        let take = expected_out.min(result[0].len());
        output.extend_from_slice(&result[0][..take]);
    }

    output
}
