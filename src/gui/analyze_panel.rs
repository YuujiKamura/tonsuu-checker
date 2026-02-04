//! Analyze panel for tonsuu-checker GUI
//!
//! Provides image selection, analysis execution, and result display.

use eframe::egui::{self, Color32, RichText, Ui};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Instant;
use tonsuu_checker::vision::{analyze_image, AnalyzerConfig};
use tonsuu_checker::config::Config;
use tonsuu_checker::vision::ai::prompts::{build_staged_analysis_prompt, GradedReferenceItem};
use tonsuu_checker::constants::get_truck_spec;
use tonsuu_checker::store::Store;
use tonsuu_checker::types::{EstimationResult, TruckClass};
use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};

/// Status message from analysis thread
#[derive(Debug, Clone)]
pub enum AnalysisStatus {
    /// Starting analysis
    Starting,
    /// Building prompt
    BuildingPrompt,
    /// Loading graded reference data
    LoadingGradedData { class: String, count: usize },
    /// Calling AI API
    CallingAI { backend: String },
    /// Staged inference progress
    StagedInference { current: usize, total: usize },
    /// Parsing response
    ParsingResponse,
    /// Merging ensemble results
    MergingResults,
    /// Completed successfully
    Completed(EstimationResult),
    /// Failed with error
    Failed(String),
}

/// Panel for analyzing dump truck images
pub struct AnalyzePanel {
    /// Currently selected image path
    selected_image: Option<PathBuf>,
    /// Analysis result (if available)
    result: Option<EstimationResult>,
    /// Error message (if any)
    error: Option<String>,
    /// Whether analysis is in progress
    is_analyzing: bool,
    /// Receiver for analysis status from background thread
    status_receiver: Option<Receiver<AnalysisStatus>>,
    /// Image path being analyzed (for saving to store)
    analyzing_path: Option<PathBuf>,
    /// Current status message
    current_status: Option<String>,
    /// Analysis start time
    start_time: Option<Instant>,
    /// Enable staged analysis with graded reference data
    use_staged_analysis: bool,
    /// Optional max capacity input (for staged analysis)
    max_capacity_input: String,
}

impl AnalyzePanel {
    /// Create a new analyze panel
    pub fn new() -> Self {
        Self {
            selected_image: None,
            result: None,
            error: None,
            is_analyzing: false,
            status_receiver: None,
            analyzing_path: None,
            current_status: None,
            start_time: None,
            use_staged_analysis: true,  // Default to staged analysis
            max_capacity_input: String::new(),
        }
    }

    /// Set image path for re-analysis (called from history panel)
    pub fn set_image_for_reanalysis(&mut self, path: PathBuf) {
        // Check if file exists
        if !path.exists() {
            self.error = Some(format!("ファイルが存在しません: {}", path.display()));
            return;
        }
        self.selected_image = Some(path);
        self.result = None;
        self.error = None;
    }

    /// Check if currently analyzing
    pub fn is_analyzing(&self) -> bool {
        self.is_analyzing
    }

    /// Trigger analysis (called externally after setting image)
    pub fn trigger_analysis(&mut self, config: &Config, store: &Store) {
        if self.selected_image.is_some() && !self.is_analyzing {
            self.start_analysis(config, store);
        }
    }

    /// Render the analyze panel UI
    pub fn ui(&mut self, ui: &mut Ui, config: &Config, store: &mut Store) {
        // Check for status updates from background thread
        self.poll_status(ui.ctx(), store);

        ui.heading("画像解析");
        ui.add_space(10.0);

        // Image selection section
        self.render_image_selection(ui);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // Analyze button and progress
        self.render_analyze_button(ui, config, store);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // Results section
        self.render_results(ui);

        // Error display
        self.render_error(ui);
    }

    /// Poll for status updates from background analysis thread
    fn poll_status(&mut self, ctx: &egui::Context, store: &mut Store) {
        if let Some(ref receiver) = self.status_receiver {
            // Drain all available messages
            loop {
                match receiver.try_recv() {
                    Ok(status) => {
                        match status {
                            AnalysisStatus::Starting => {
                                self.current_status = Some("解析を開始しています...".to_string());
                            }
                            AnalysisStatus::BuildingPrompt => {
                                self.current_status = Some("プロンプトを構築中...".to_string());
                            }
                            AnalysisStatus::LoadingGradedData { class, count } => {
                                self.current_status = Some(format!(
                                    "{}クラスの実測データ{}件を参照中...",
                                    class, count
                                ));
                            }
                            AnalysisStatus::CallingAI { backend } => {
                                self.current_status = Some(format!("AI ({}) に問い合わせ中...", backend));
                            }
                            AnalysisStatus::StagedInference { current, total } => {
                                self.current_status = Some(format!(
                                    "推論 {}/{} 実行中...",
                                    current, total
                                ));
                            }
                            AnalysisStatus::ParsingResponse => {
                                self.current_status = Some("応答を解析中...".to_string());
                            }
                            AnalysisStatus::MergingResults => {
                                self.current_status = Some("結果を統合中...".to_string());
                            }
                            AnalysisStatus::Completed(result) => {
                                // Save to history store
                                if let Some(ref path) = self.analyzing_path {
                                    if let Err(e) = store.add_analysis(path, result.clone()) {
                                        self.error = Some(format!("履歴の保存に失敗しました: {}", e));
                                    }
                                }
                                self.result = Some(result);
                                self.is_analyzing = false;
                                self.status_receiver = None;
                                self.analyzing_path = None;
                                self.current_status = None;
                                self.start_time = None;
                                return;
                            }
                            AnalysisStatus::Failed(e) => {
                                self.error = Some(format!("解析エラー: {}", e));
                                self.is_analyzing = false;
                                self.status_receiver = None;
                                self.analyzing_path = None;
                                self.current_status = None;
                                self.start_time = None;
                                return;
                            }
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // No more messages, request repaint to check again
                        ctx.request_repaint();
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.error = Some("解析スレッドが異常終了しました".to_string());
                        self.is_analyzing = false;
                        self.status_receiver = None;
                        self.analyzing_path = None;
                        self.current_status = None;
                        self.start_time = None;
                        return;
                    }
                }
            }
        }
    }

    /// Render the image selection section
    fn render_image_selection(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let enabled = !self.is_analyzing;
            if ui.add_enabled(enabled, egui::Button::new("画像を選択...")).clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("画像ファイル", &["jpg", "jpeg", "png", "gif", "bmp", "webp"])
                    .pick_file()
                {
                    self.selected_image = Some(path);
                    // Clear previous results when new image is selected
                    self.result = None;
                    self.error = None;
                }
            }

            ui.add_space(10.0);

            // Display selected image path
            if let Some(ref path) = self.selected_image {
                ui.label(
                    RichText::new(path.display().to_string())
                        .monospace()
                        .color(Color32::LIGHT_BLUE),
                );
            } else {
                ui.label(
                    RichText::new("画像が選択されていません")
                        .italics()
                        .color(Color32::GRAY),
                );
            }
        });

        // Show image preview path info
        if let Some(ref path) = self.selected_image {
            ui.add_space(5.0);
            if let Some(file_name) = path.file_name() {
                ui.label(format!("ファイル名: {}", file_name.to_string_lossy()));
            }
        }
    }

    /// Render the analyze button and progress
    fn render_analyze_button(&mut self, ui: &mut Ui, config: &Config, store: &Store) {
        let can_analyze = self.selected_image.is_some() && !self.is_analyzing;

        // Staged analysis options
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.use_staged_analysis, "段階的解析");
            ui.add_space(10.0);
            if self.use_staged_analysis {
                ui.label("最大積載量:");
                ui.add(egui::TextEdit::singleline(&mut self.max_capacity_input)
                    .desired_width(60.0)
                    .hint_text("例: 10"));
                ui.label("t");
                ui.add_space(5.0);
                ui.label(
                    RichText::new("(空欄で自動推定)")
                        .color(Color32::GRAY)
                        .small()
                );
            }
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            let button_text = if self.is_analyzing {
                "解析中..."
            } else {
                "解析"
            };

            let button = egui::Button::new(RichText::new(button_text).size(16.0));

            if ui.add_enabled(can_analyze, button).clicked() {
                self.start_analysis(config, store);
            }

            if self.is_analyzing {
                ui.spinner();
            }
        });

        // Show detailed progress
        if self.is_analyzing {
            ui.add_space(8.0);

            // Progress box
            egui::Frame::new()
                .fill(Color32::from_gray(30))
                .inner_margin(10.0)
                .corner_radius(4.0)
                .show(ui, |ui| {
                    // Elapsed time
                    if let Some(start) = self.start_time {
                        let elapsed = start.elapsed().as_secs_f32();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("経過時間:").strong());
                            ui.label(format!("{:.1} 秒", elapsed));
                        });
                    }

                    // Current status
                    if let Some(ref status) = self.current_status {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("状態:").strong());
                            ui.label(RichText::new(status).color(Color32::LIGHT_BLUE));
                        });
                    }

                    // Backend info
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("バックエンド:").strong());
                        ui.label(&config.backend);
                    });

                    // Image being analyzed
                    if let Some(ref path) = self.analyzing_path {
                        if let Some(filename) = path.file_name() {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("対象:").strong());
                                ui.label(filename.to_string_lossy().to_string());
                            });
                        }
                    }
                });
        }
    }

    /// Start analysis in a background thread
    fn start_analysis(&mut self, config: &Config, store: &Store) {
        let Some(ref image_path) = self.selected_image else {
            return;
        };

        self.is_analyzing = true;
        self.error = None;
        self.result = None;
        self.analyzing_path = Some(image_path.clone());
        self.start_time = Some(Instant::now());
        self.current_status = Some("準備中...".to_string());

        // Create channel for status updates
        let (sender, receiver): (Sender<AnalysisStatus>, Receiver<AnalysisStatus>) = channel();
        self.status_receiver = Some(receiver);

        // Clone data for thread
        let image_path = image_path.clone();
        let backend = config.backend.clone();
        let model = config.model.clone();
        let ensemble_count = config.ensemble_count;
        let use_staged = self.use_staged_analysis;

        // Parse max capacity if provided
        let max_capacity: Option<f64> = self.max_capacity_input.trim()
            .parse()
            .ok()
            .filter(|&v: &f64| v > 0.0);

        // Load graded reference data before starting thread (if staged analysis)
        let graded_references: Vec<GradedReferenceItem> = if use_staged {
            // If we have a max capacity, load graded data for that truck class
            if let Some(cap) = max_capacity {
                let truck_class = TruckClass::from_capacity(cap);
                if truck_class != TruckClass::Unknown {
                    store.select_stock_by_grade(truck_class)
                        .iter()
                        .map(|g| GradedReferenceItem {
                            grade_name: g.grade.label().to_string(),
                            actual_tonnage: g.entry.actual_tonnage.unwrap_or(0.0),
                            max_capacity: g.entry.max_capacity.unwrap_or(0.0),
                            load_ratio: g.load_ratio,
                            memo: g.entry.notes.clone(),
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()  // Will be loaded after first inference
            }
        } else {
            Vec::new()
        };

        // Spawn analysis thread
        thread::spawn(move || {
            // Send starting status
            let _ = sender.send(AnalysisStatus::Starting);

            if use_staged {
                run_staged_analysis(
                    sender,
                    image_path,
                    backend,
                    model,
                    max_capacity,
                    graded_references,
                    ensemble_count,
                );
            } else {
                run_simple_analysis(sender, image_path, backend, model);
            }
        });
    }
}

/// Run simple (non-staged) analysis
fn run_simple_analysis(
    sender: Sender<AnalysisStatus>,
    image_path: PathBuf,
    backend: String,
    model: Option<String>,
) {
    let _ = sender.send(AnalysisStatus::BuildingPrompt);

    let analyzer_config = AnalyzerConfig::default()
        .with_backend(&backend)
        .with_model(model);

    let _ = sender.send(AnalysisStatus::CallingAI { backend: backend.clone() });

    let result = analyze_image(&image_path, &analyzer_config);

    let _ = sender.send(AnalysisStatus::ParsingResponse);

    match result {
        Ok(estimation) => {
            let _ = sender.send(AnalysisStatus::Completed(estimation));
        }
        Err(e) => {
            let _ = sender.send(AnalysisStatus::Failed(e.to_string()));
        }
    }
}

/// Run staged analysis with graded reference data
fn run_staged_analysis(
    sender: Sender<AnalysisStatus>,
    image_path: PathBuf,
    backend: String,
    model: Option<String>,
    max_capacity: Option<f64>,
    graded_references: Vec<GradedReferenceItem>,
    ensemble_count: u32,
) {
    let _ = sender.send(AnalysisStatus::BuildingPrompt);

    let target_count = ensemble_count.max(1) as usize;
    let mut results: Vec<EstimationResult> = Vec::new();

    // Notify if we have graded data
    if !graded_references.is_empty() {
        if let Some(cap) = max_capacity {
            let class = TruckClass::from_capacity(cap);
            let _ = sender.send(AnalysisStatus::LoadingGradedData {
                class: class.label().to_string(),
                count: graded_references.len(),
            });
        }
    }

    // Configure backend
    let ai_backend = match backend.to_lowercase().as_str() {
        "claude" => Backend::Claude,
        "codex" => Backend::Codex,
        _ => Backend::Gemini,
    };

    for iteration in 0..target_count {
        let _ = sender.send(AnalysisStatus::StagedInference {
            current: iteration + 1,
            total: target_count,
        });

        // Build prompt with graded data
        let prompt = build_staged_analysis_prompt(max_capacity, &graded_references);

        // Configure AI options
        let mut ai_options = if let Some(ref m) = model {
            AnalyzeOptions::with_model(m)
        } else {
            AnalyzeOptions::default()
        };
        ai_options = ai_options.with_backend(ai_backend).json();

        let _ = sender.send(AnalysisStatus::CallingAI { backend: backend.clone() });

        // Call AI
        match analyze(&prompt, &[image_path.clone()], ai_options) {
            Ok(response) => {
                let _ = sender.send(AnalysisStatus::ParsingResponse);
                match parse_ai_response(&response) {
                    Ok(result) => {
                        // After first iteration with no max_capacity, we could
                        // potentially detect truck class and load graded data
                        // But since we don't have Store access here, we skip this
                        // The initial graded_references from main thread is used
                        results.push(result);
                    }
                    Err(e) => {
                        eprintln!("Inference {} parse error: {}", iteration + 1, e);
                    }
                }
            }
            Err(e) => {
                eprintln!("Inference {} error: {}", iteration + 1, e);
            }
        }
    }

    if results.is_empty() {
        let _ = sender.send(AnalysisStatus::Failed(
            "All inference attempts failed".to_string()
        ));
        return;
    }

    // Merge results
    let _ = sender.send(AnalysisStatus::MergingResults);
    let merged = merge_estimation_results(&results);
    let _ = sender.send(AnalysisStatus::Completed(merged));
}

/// Parse AI response into EstimationResult
fn parse_ai_response(response: &str) -> Result<EstimationResult, String> {
    let json_str = extract_json_from_response(response);
    let mut result: EstimationResult = serde_json::from_str(&json_str).map_err(|e| {
        let truncated: String = response.chars().take(500).collect();
        format!("Failed to parse AI response: {}. Response: {}", e, truncated)
    })?;

    // Calculate volume and tonnage if not provided by AI
    if result.estimated_volume_m3 == 0.0 || result.estimated_tonnage == 0.0 {
        calculate_volume_and_tonnage(&mut result);
    }

    Ok(result)
}

/// Calculate volume and tonnage from estimated parameters
fn calculate_volume_and_tonnage(result: &mut EstimationResult) {
    const LOWER_AREA: f64 = 6.8; // 4tダンプ底面積 (m²)

    let density = match result.material_type.as_str() {
        s if s.contains("土砂") => 1.8,
        _ => 2.5,
    };

    let upper_area = result.upper_area.unwrap_or(LOWER_AREA);
    let height = result.height.unwrap_or(0.0);
    let void_ratio = result.void_ratio.unwrap_or(0.35);

    if height > 0.0 {
        let volume = (upper_area + LOWER_AREA) / 2.0 * height;
        let tonnage = volume * density * (1.0 - void_ratio);
        result.estimated_volume_m3 = (volume * 100.0).round() / 100.0;
        result.estimated_tonnage = (tonnage * 100.0).round() / 100.0;
    }
}

/// Extract JSON from response (handles markdown code blocks)
fn extract_json_from_response(response: &str) -> String {
    let response = response.trim();

    // Check for markdown code block
    if response.starts_with("```json") {
        if let Some(end) = response.rfind("```") {
            let start = response.find('\n').unwrap_or(7) + 1;
            if start < end {
                return response[start..end].trim().to_string();
            }
        }
    }

    // Check for generic code block
    if response.starts_with("```") {
        if let Some(end) = response.rfind("```") {
            let start = response.find('\n').unwrap_or(3) + 1;
            if start < end {
                return response[start..end].trim().to_string();
            }
        }
    }

    // Try to find JSON object directly
    if let Some(start) = response.find('{') {
        if let Some(end) = response.rfind('}') {
            if start < end {
                return response[start..=end].to_string();
            }
        }
    }

    response.to_string()
}

/// Merge multiple estimation results (ensemble voting)
fn merge_estimation_results(results: &[EstimationResult]) -> EstimationResult {
    use std::collections::HashMap;

    if results.is_empty() {
        return EstimationResult::default();
    }

    if results.len() == 1 {
        return results[0].clone();
    }

    // Average numeric values
    let avg_volume: f64 = results.iter().map(|r| r.estimated_volume_m3).sum::<f64>()
        / results.len() as f64;
    let avg_tonnage: f64 =
        results.iter().map(|r| r.estimated_tonnage).sum::<f64>() / results.len() as f64;
    let avg_confidence: f64 =
        results.iter().map(|r| r.confidence_score).sum::<f64>() / results.len() as f64;

    // Mode for categorical values
    fn mode_string(values: Vec<String>) -> String {
        let mut counts: HashMap<String, usize> = HashMap::new();
        for v in values.iter() {
            *counts.entry(v.clone()).or_insert(0) += 1;
        }
        counts.into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(value, _)| value)
            .unwrap_or_default()
    }

    let truck_type = mode_string(results.iter().map(|r| r.truck_type.clone()).collect());
    let material_type = mode_string(results.iter().map(|r| r.material_type.clone()).collect());

    // Use first result as base
    let mut merged = results[0].clone();
    merged.truck_type = truck_type;
    merged.material_type = material_type;
    merged.estimated_volume_m3 = (avg_volume * 100.0).round() / 100.0;
    merged.estimated_tonnage = (avg_tonnage * 100.0).round() / 100.0;
    merged.confidence_score = avg_confidence;
    merged.ensemble_count = Some(results.len() as u32);
    merged.reasoning = format!(
        "【統合推論】有効サンプル:{}/{}。{}",
        results.len(),
        results.len(),
        merged.reasoning
    );

    merged
}

impl AnalyzePanel {
    /// Render the analysis results
    fn render_results(&self, ui: &mut Ui) {
        ui.label(RichText::new("解析結果").strong().size(14.0));
        ui.add_space(5.0);

        if let Some(ref result) = self.result {
            if !result.is_target_detected {
                ui.label(
                    RichText::new("対象が検出されませんでした")
                        .color(Color32::YELLOW)
                        .italics(),
                );
                if !result.reasoning.is_empty() {
                    ui.add_space(5.0);
                    ui.label(format!("理由: {}", result.reasoning));
                }
                return;
            }

            // Display results in a grid for alignment
            egui::Grid::new("result_grid")
                .num_columns(2)
                .spacing([20.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    // Truck type
                    ui.label(RichText::new("トラック種別:").strong());
                    ui.label(&result.truck_type);
                    ui.end_row();

                    // License plate (if available)
                    if let Some(ref plate) = result.license_plate {
                        ui.label(RichText::new("ナンバープレート:").strong());
                        ui.label(plate);
                        ui.end_row();
                    }

                    // License number (if available)
                    if let Some(ref number) = result.license_number {
                        ui.label(RichText::new("車両番号:").strong());
                        ui.label(number);
                        ui.end_row();
                    }

                    // Material type
                    ui.label(RichText::new("材料:").strong());
                    ui.label(&result.material_type);
                    ui.end_row();

                    // Estimated volume
                    ui.label(RichText::new("推定容量:").strong());
                    ui.label(format!("{:.2} m\u{00B3}", result.estimated_volume_m3));
                    ui.end_row();

                    // Max capacity from truck spec (if available)
                    if let Some(spec) = get_truck_spec(&result.truck_type) {
                        ui.label(RichText::new("最大積載量:").strong());
                        ui.label(format!("{:.2} t", spec.max_capacity));
                        ui.end_row();
                    }

                    // Estimated tonnage
                    ui.label(RichText::new("推定重量:").strong());
                    ui.label(
                        RichText::new(format!("{:.2} t", result.estimated_tonnage))
                            .color(Color32::LIGHT_GREEN)
                            .strong(),
                    );
                    ui.end_row();

                    // Confidence score
                    ui.label(RichText::new("信頼度:").strong());
                    let confidence_pct = result.confidence_score * 100.0;
                    let confidence_color = if confidence_pct >= 80.0 {
                        Color32::LIGHT_GREEN
                    } else if confidence_pct >= 60.0 {
                        Color32::YELLOW
                    } else {
                        Color32::LIGHT_RED
                    };
                    ui.label(
                        RichText::new(format!("{:.1}%", confidence_pct)).color(confidence_color),
                    );
                    ui.end_row();

                    // Ensemble count (if available)
                    if let Some(count) = result.ensemble_count {
                        ui.label(RichText::new("アンサンブル数:").strong());
                        ui.label(format!("{}", count));
                        ui.end_row();
                    }
                });

            // Material breakdown (if available)
            if !result.material_breakdown.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("材料内訳:").strong());
                egui::Grid::new("material_breakdown_grid")
                    .num_columns(3)
                    .spacing([15.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("材料").underline());
                        ui.label(RichText::new("割合").underline());
                        ui.label(RichText::new("密度").underline());
                        ui.end_row();

                        for breakdown in &result.material_breakdown {
                            ui.label(&breakdown.material);
                            ui.label(format!("{:.1}%", breakdown.percentage));
                            ui.label(format!("{:.2} t/m\u{00B3}", breakdown.density));
                            ui.end_row();
                        }
                    });
            }

            // Reasoning
            if !result.reasoning.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new("理由:").strong());
                ui.add_space(3.0);

                egui::Frame::new()
                    .fill(Color32::from_gray(40))
                    .inner_margin(8.0)
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        ui.label(&result.reasoning);
                    });
            }
        } else if !self.is_analyzing {
            ui.label(
                RichText::new("画像を選択して「解析」ボタンを押してください")
                    .italics()
                    .color(Color32::GRAY),
            );
        }
    }

    /// Render error messages
    fn render_error(&self, ui: &mut Ui) {
        if let Some(ref error) = self.error {
            ui.add_space(10.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(80, 20, 20))
                .inner_margin(8.0)
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(error).color(Color32::LIGHT_RED));
                });
        }
    }
}

impl Default for AnalyzePanel {
    fn default() -> Self {
        Self::new()
    }
}
