use anyhow::{anyhow, Context, Result};
use ort::session::Session;
use ort::value::Tensor;

const CHUNK_SIZE: usize = 512; // 32ms at 16kHz
const SAMPLE_RATE: i64 = 16000;
const STATE_SIZE: usize = 2 * 1 * 128; // [2, 1, 128] flattened

pub struct SileroVad {
    session: Session,
    h: Vec<f32>,
    c: Vec<f32>,
    threshold: f32,
}

impl SileroVad {
    pub fn new(model_path: &str) -> Result<Self> {
        let session = Session::builder()
            .map_err(|e| anyhow!("{e}"))?
            .with_intra_threads(1)
            .map_err(|e| anyhow!("{e}"))?
            .commit_from_file(model_path)
            .context("Failed to load Silero VAD model")?;

        Ok(Self {
            session,
            h: vec![0.0f32; STATE_SIZE],
            c: vec![0.0f32; STATE_SIZE],
            threshold: 0.5,
        })
    }

    /// Process a 512-sample chunk and return speech probability.
    pub fn process_chunk(&mut self, chunk: &[f32]) -> Result<f32> {
        assert_eq!(
            chunk.len(),
            CHUNK_SIZE,
            "VAD chunk must be {} samples",
            CHUNK_SIZE
        );

        let input = Tensor::from_array(([1usize, CHUNK_SIZE], chunk.to_vec()))?;
        let sr = Tensor::from_array(([1usize], vec![SAMPLE_RATE]))?;
        let h = Tensor::from_array(([2usize, 1, 128], self.h.clone()))?;
        let c = Tensor::from_array(([2usize, 1, 128], self.c.clone()))?;

        let outputs = self.session.run(ort::inputs! {
            "input" => input,
            "sr" => sr,
            "h" => h,
            "c" => c,
        })?;

        // Extract speech probability
        let prob = {
            let (_, data) = outputs["output"].try_extract_tensor::<f32>()?;
            data[0]
        };

        // Update hidden states
        {
            let (_, data) = outputs["hn"].try_extract_tensor::<f32>()?;
            self.h.copy_from_slice(data);
        }
        {
            let (_, data) = outputs["cn"].try_extract_tensor::<f32>()?;
            self.c.copy_from_slice(data);
        }

        Ok(prob)
    }

    pub fn is_speech(&mut self, chunk: &[f32]) -> Result<bool> {
        let prob = self.process_chunk(chunk)?;
        Ok(prob > self.threshold)
    }

    pub fn reset(&mut self) {
        self.h.fill(0.0);
        self.c.fill(0.0);
    }

    pub fn chunk_size() -> usize {
        CHUNK_SIZE
    }
}
