//! CLI-based AiBackend implementation using cli-ai-analyzer
//!
//! Uses file paths directly to avoid redundant read→write round-trips.
//! The `images` parameter from AiBackend::send_prompt is ignored;
//! instead, the original file paths are passed directly to cli-ai-analyzer.

use cli_ai_analyzer::{analyze, AnalyzeOptions};
use tonsuu_core::pipeline::{AiBackend, PipelineError};
use std::path::PathBuf;

/// AiBackend implementation that uses cli-ai-analyzer CLI tools.
///
/// Holds the original image file paths so they can be passed directly
/// to the AI backend without copying data through temp files.
pub struct CliAiBackend {
    pub options: AnalyzeOptions,
    pub image_paths: Vec<PathBuf>,
}

impl AiBackend for CliAiBackend {
    fn send_prompt(&self, prompt: &str, _images: &[Vec<u8>]) -> Result<String, PipelineError> {
        analyze(prompt, &self.image_paths, self.options.clone())
            .map_err(|e| PipelineError::AiError(e.to_string()))
    }
}
