//! Vehicle management panel for tonsuu-checker GUI

use eframe::egui::{self, Color32, RichText, Ui};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use tonsuu_app::config::Config;
use tonsuu_store::VehicleStore;
use tonsuu_types::{RegisteredVehicle, TruckClass};
use cli_ai_analyzer::{analyze, AnalyzeOptions, Backend};

/// Scanned vehicle folder information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ScannedVehicleFolder {
    /// Folder name (will be used for vehicle name)
    pub folder_name: String,
    /// Full path to the folder
    pub folder_path: PathBuf,
    /// Detected vehicle registration certificate images (車検証)
    pub shaken_images: Vec<PathBuf>,
    /// Detected vehicle photos
    pub photo_images: Vec<PathBuf>,
}

/// Result of folder scanning
#[derive(Debug, Clone)]
pub struct FolderScanResult {
    /// Root folder path
    pub root_path: PathBuf,
    /// Scanned vehicle folders
    pub folders: Vec<ScannedVehicleFolder>,
}

/// Status message from processing thread
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ProcessStatus {
    /// Scanning folders
    Scanning,
    /// Processing vehicle
    Processing { current: usize, total: usize, name: String },
    /// Analyzing 車検証
    AnalyzingShaken { name: String },
    /// Registering vehicle
    Registering { name: String },
    /// Single vehicle completed
    VehicleCompleted { name: String, success: bool, error: Option<String> },
    /// All processing completed
    Completed { success_count: usize, fail_count: usize },
    /// Error occurred
    Error(String),
}

/// Result of a single vehicle processing
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct VehicleProcessResult {
    pub folder_name: String,
    pub success: bool,
    pub error: Option<String>,
    pub vehicle_name: Option<String>,
    pub capacity: Option<f64>,
}

/// Panel for managing registered vehicles
pub struct VehiclePanel {
    /// New vehicle form fields
    new_name: String,
    new_capacity: String,
    new_plate: String,
    new_notes: String,
    new_image_path: Option<PathBuf>,
    /// Status message
    status_message: Option<(String, bool)>, // (message, is_error)
    /// Selected vehicle ID for details
    #[allow(dead_code)]
    selected_id: Option<String>,
    /// Folder scan result
    scan_result: Option<FolderScanResult>,
    /// Whether scanning is in progress
    is_scanning: bool,
    /// Whether processing is in progress
    is_processing: bool,
    /// Processing progress (current, total)
    process_progress: (usize, usize),
    /// Current processing status message
    process_status: Option<String>,
    /// Receiver for processing status from background thread
    status_receiver: Option<Receiver<ProcessStatus>>,
    /// Processing results for summary display
    process_results: Vec<VehicleProcessResult>,
    /// Vehicles to register (sent from processing thread)
    vehicles_to_register: Option<Receiver<RegisteredVehicle>>,
}

impl VehiclePanel {
    pub fn new() -> Self {
        Self {
            new_name: String::new(),
            new_capacity: String::new(),
            new_plate: String::new(),
            new_notes: String::new(),
            new_image_path: None,
            status_message: None,
            selected_id: None,
            scan_result: None,
            is_scanning: false,
            is_processing: false,
            process_progress: (0, 0),
            process_status: None,
            status_receiver: None,
            process_results: Vec::new(),
            vehicles_to_register: None,
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, vehicle_store: &mut VehicleStore, config: &Config) {
        // Poll for status updates from background thread
        self.poll_status(ui.ctx(), vehicle_store);

        ui.heading("車両管理");
        ui.add_space(10.0);

        // Auto-collect section
        self.render_auto_collect_section(ui, config);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // Add vehicle form
        self.render_add_form(ui, vehicle_store);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        // Vehicle list
        self.render_vehicle_list(ui, vehicle_store);

        // Status message
        if let Some((ref msg, is_error)) = self.status_message {
            ui.add_space(10.0);
            let color = if is_error {
                Color32::LIGHT_RED
            } else {
                Color32::LIGHT_GREEN
            };
            ui.label(RichText::new(msg).color(color));
        }
    }

    /// Poll for status updates from background processing thread
    fn poll_status(&mut self, ctx: &egui::Context, vehicle_store: &mut VehicleStore) {
        // Check for vehicles to register
        if let Some(ref receiver) = self.vehicles_to_register {
            loop {
                match receiver.try_recv() {
                    Ok(vehicle) => {
                        if let Err(e) = vehicle_store.add_vehicle(vehicle) {
                            eprintln!("Failed to register vehicle: {}", e);
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.vehicles_to_register = None;
                        break;
                    }
                }
            }
        }

        // Check for status updates
        if let Some(ref receiver) = self.status_receiver {
            loop {
                match receiver.try_recv() {
                    Ok(status) => {
                        match status {
                            ProcessStatus::Scanning => {
                                self.process_status = Some("フォルダをスキャン中...".to_string());
                            }
                            ProcessStatus::Processing { current, total, name } => {
                                self.process_progress = (current, total);
                                self.process_status = Some(format!("処理中: {} ({}/{})", name, current, total));
                            }
                            ProcessStatus::AnalyzingShaken { name } => {
                                self.process_status = Some(format!("車検証を解析中: {}", name));
                            }
                            ProcessStatus::Registering { name } => {
                                self.process_status = Some(format!("登録中: {}", name));
                            }
                            ProcessStatus::VehicleCompleted { name, success, error } => {
                                self.process_results.push(VehicleProcessResult {
                                    folder_name: name.clone(),
                                    success,
                                    error,
                                    vehicle_name: Some(name),
                                    capacity: None,
                                });
                            }
                            ProcessStatus::Completed { success_count, fail_count } => {
                                self.is_processing = false;
                                self.status_receiver = None;
                                self.vehicles_to_register = None;
                                self.process_status = Some(format!(
                                    "完了: {}件成功, {}件失敗",
                                    success_count, fail_count
                                ));
                                self.status_message = Some((
                                    format!("一括登録完了: {}件成功, {}件失敗", success_count, fail_count),
                                    fail_count > 0,
                                ));
                                return;
                            }
                            ProcessStatus::Error(e) => {
                                self.is_processing = false;
                                self.status_receiver = None;
                                self.vehicles_to_register = None;
                                self.process_status = Some(format!("エラー: {}", e));
                                self.status_message = Some((format!("エラー: {}", e), true));
                                return;
                            }
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint();
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        self.is_processing = false;
                        self.status_receiver = None;
                        self.vehicles_to_register = None;
                        return;
                    }
                }
            }
        }
    }

    /// Render auto-collect section
    fn render_auto_collect_section(&mut self, ui: &mut Ui, config: &Config) {
        ui.label(RichText::new("フォルダから一括登録").strong());
        ui.add_space(5.0);

        ui.horizontal(|ui| {
            let enabled = !self.is_scanning && !self.is_processing;
            if ui.add_enabled(enabled, egui::Button::new("フォルダから一括登録...")).clicked() {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.scan_folder(&path);
                }
            }

            if self.is_scanning {
                ui.spinner();
                ui.label("スキャン中...");
            }
        });

        // Show scan results - extract data to avoid borrow issues
        let scan_info = self.scan_result.as_ref().map(|result| {
            let root_path_display = result.root_path.display().to_string();
            let folder_count = result.folders.len();
            let folders_preview: Vec<_> = result.folders.iter().map(|f| {
                (
                    f.folder_name.clone(),
                    f.shaken_images.len(),
                    f.photo_images.len(),
                )
            }).collect();
            (root_path_display, folder_count, folders_preview)
        });

        if let Some((root_path_display, folder_count, folders_preview)) = scan_info {
            ui.add_space(8.0);

            egui::Frame::new()
                .fill(Color32::from_gray(30))
                .inner_margin(10.0)
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.label(RichText::new(format!(
                        "スキャン結果: {}",
                        root_path_display
                    )).color(Color32::LIGHT_BLUE));
                    ui.add_space(5.0);

                    if folder_count == 0 {
                        ui.label(RichText::new("車両フォルダが見つかりませんでした").color(Color32::YELLOW));
                    } else {
                        ui.label(format!("{}件の車両フォルダを検出", folder_count));
                        ui.add_space(5.0);

                        // Preview list
                        egui::ScrollArea::vertical()
                            .max_height(150.0)
                            .show(ui, |ui| {
                                egui::Grid::new("scan_result_grid")
                                    .num_columns(4)
                                    .spacing([10.0, 4.0])
                                    .striped(true)
                                    .show(ui, |ui| {
                                        // Header
                                        ui.label(RichText::new("フォルダ名").strong());
                                        ui.label(RichText::new("車検証").strong());
                                        ui.label(RichText::new("写真").strong());
                                        ui.label(RichText::new("状態").strong());
                                        ui.end_row();

                                        for (folder_name, shaken_count, photo_count) in &folders_preview {
                                            ui.label(folder_name);
                                            ui.label(format!("{}枚", shaken_count));
                                            ui.label(format!("{}枚", photo_count));

                                            let status = if *shaken_count == 0 {
                                                RichText::new("車検証なし").color(Color32::YELLOW)
                                            } else if *photo_count == 0 {
                                                RichText::new("写真なし").color(Color32::YELLOW)
                                            } else {
                                                RichText::new("OK").color(Color32::LIGHT_GREEN)
                                            };
                                            ui.label(status);
                                            ui.end_row();
                                        }
                                    });
                            });

                        ui.add_space(8.0);

                        // Action buttons
                        let can_process = !self.is_processing && folder_count > 0;
                        ui.horizontal(|ui| {
                            if ui.add_enabled(can_process, egui::Button::new("解析して登録")).clicked() {
                                self.start_processing(config);
                            }

                            if ui.button("クリア").clicked() {
                                self.scan_result = None;
                                self.process_results.clear();
                            }

                            if self.is_processing {
                                ui.spinner();
                            }
                        });
                    }
                });
        }

        // Processing progress
        if self.is_processing {
            ui.add_space(8.0);

            egui::Frame::new()
                .fill(Color32::from_gray(25))
                .inner_margin(10.0)
                .corner_radius(4.0)
                .show(ui, |ui| {
                    let (current, total) = self.process_progress;
                    if total > 0 {
                        let progress = current as f32 / total as f32;
                        ui.add(egui::ProgressBar::new(progress).show_percentage());
                    }

                    if let Some(ref status) = self.process_status {
                        ui.label(RichText::new(status).color(Color32::LIGHT_BLUE));
                    }
                });
        }

        // Results summary
        if !self.process_results.is_empty() && !self.is_processing {
            ui.add_space(8.0);

            let success_count = self.process_results.iter().filter(|r| r.success).count();
            let fail_count = self.process_results.iter().filter(|r| !r.success).count();

            egui::Frame::new()
                .fill(Color32::from_gray(30))
                .inner_margin(10.0)
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.label(RichText::new("処理結果").strong());
                    ui.add_space(5.0);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("成功: {}件", success_count)).color(Color32::LIGHT_GREEN));
                        ui.label(RichText::new(format!("失敗: {}件", fail_count)).color(Color32::LIGHT_RED));
                    });

                    // Show failed items
                    let failed: Vec<_> = self.process_results.iter().filter(|r| !r.success).collect();
                    if !failed.is_empty() {
                        ui.add_space(5.0);
                        ui.label(RichText::new("失敗した項目:").color(Color32::YELLOW));
                        for result in failed {
                            let error_msg = result.error.as_deref().unwrap_or("不明なエラー");
                            ui.label(format!("  - {}: {}", result.folder_name, error_msg));
                        }
                    }
                });
        }
    }

    /// Scan folder for vehicle subfolders
    fn scan_folder(&mut self, root_path: &PathBuf) {
        self.is_scanning = true;
        self.scan_result = None;
        self.process_results.clear();

        // Scan synchronously (it's fast enough)
        let mut folders = Vec::new();

        if let Ok(entries) = std::fs::read_dir(root_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    let (shaken_images, photo_images) = scan_vehicle_folder(&path);

                    // Only include folders with at least some images
                    if !shaken_images.is_empty() || !photo_images.is_empty() {
                        folders.push(ScannedVehicleFolder {
                            folder_name,
                            folder_path: path,
                            shaken_images,
                            photo_images,
                        });
                    }
                }
            }
        }

        // Sort by folder name
        folders.sort_by(|a, b| a.folder_name.cmp(&b.folder_name));

        self.scan_result = Some(FolderScanResult {
            root_path: root_path.clone(),
            folders,
        });
        self.is_scanning = false;
    }

    /// Start processing scanned folders
    fn start_processing(&mut self, config: &Config) {
        let Some(ref scan_result) = self.scan_result else {
            return;
        };

        self.is_processing = true;
        self.process_results.clear();
        self.process_progress = (0, scan_result.folders.len());
        self.process_status = Some("処理を開始しています...".to_string());

        // Create channels
        let (status_tx, status_rx): (Sender<ProcessStatus>, Receiver<ProcessStatus>) = channel();
        let (vehicle_tx, vehicle_rx): (Sender<RegisteredVehicle>, Receiver<RegisteredVehicle>) = channel();
        self.status_receiver = Some(status_rx);
        self.vehicles_to_register = Some(vehicle_rx);

        // Clone data for thread
        let folders = scan_result.folders.clone();
        let backend = config.backend.clone();
        let model = config.model.clone();

        // Spawn processing thread
        thread::spawn(move || {
            process_vehicle_folders(folders, backend, model, status_tx, vehicle_tx);
        });
    }

    fn render_add_form(&mut self, ui: &mut Ui, vehicle_store: &mut VehicleStore) {
        ui.label(RichText::new("新規車両登録").strong());
        ui.add_space(5.0);

        egui::Grid::new("add_vehicle_form")
            .num_columns(2)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                // Vehicle name
                ui.label("車両名:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_name)
                        .hint_text("例: 日野 プロフィア")
                        .desired_width(200.0),
                );
                ui.end_row();

                // Max capacity
                ui.label("最大積載量:");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_capacity)
                            .hint_text("例: 10.0")
                            .desired_width(80.0),
                    );
                    ui.label("t");

                    // Show truck class preview
                    if let Ok(cap) = self.new_capacity.parse::<f64>() {
                        let class = TruckClass::from_capacity(cap);
                        ui.label(
                            RichText::new(format!("→ {}クラス", class.label()))
                                .color(Color32::LIGHT_BLUE),
                        );
                    }
                });
                ui.end_row();

                // License plate
                ui.label("ナンバー:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_plate)
                        .hint_text("例: 品川 100 あ 1234")
                        .desired_width(200.0),
                );
                ui.end_row();

                // Image selection
                ui.label("車両画像:");
                ui.horizontal(|ui| {
                    if ui.button("画像を選択...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("画像", &["jpg", "jpeg", "png", "gif", "bmp", "webp"])
                            .pick_file()
                        {
                            self.new_image_path = Some(path);
                        }
                    }
                    if let Some(ref path) = self.new_image_path {
                        if let Some(name) = path.file_name() {
                            ui.label(
                                RichText::new(name.to_string_lossy().to_string())
                                    .color(Color32::LIGHT_GREEN),
                            );
                        }
                        if ui.small_button("✕").clicked() {
                            self.new_image_path = None;
                        }
                    } else {
                        ui.label(RichText::new("(必須)").color(Color32::YELLOW));
                    }
                });
                ui.end_row();

                // Notes
                ui.label("メモ:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_notes)
                        .hint_text("任意のメモ")
                        .desired_width(200.0),
                );
                ui.end_row();
            });

        ui.add_space(8.0);

        // Add button
        let can_add = !self.new_name.trim().is_empty()
            && self.new_capacity.parse::<f64>().is_ok()
            && self.new_image_path.is_some();

        if ui
            .add_enabled(can_add, egui::Button::new("追加"))
            .clicked()
        {
            self.add_vehicle(vehicle_store);
        }
    }

    fn add_vehicle(&mut self, vehicle_store: &mut VehicleStore) {
        let capacity: f64 = match self.new_capacity.parse() {
            Ok(v) if v > 0.0 => v,
            _ => {
                self.status_message = Some(("積載量が不正です".to_string(), true));
                return;
            }
        };

        let image_path = match &self.new_image_path {
            Some(p) => p.display().to_string(),
            None => {
                self.status_message = Some(("画像を選択してください".to_string(), true));
                return;
            }
        };

        // Create thumbnail (base64)
        let thumbnail = create_thumbnail(&image_path);

        let mut vehicle = RegisteredVehicle::new(self.new_name.trim().to_string(), capacity)
            .with_image(image_path, thumbnail);

        if !self.new_plate.trim().is_empty() {
            vehicle = vehicle.with_license_plate(self.new_plate.trim().to_string());
        }

        if !self.new_notes.trim().is_empty() {
            vehicle.notes = Some(self.new_notes.trim().to_string());
        }

        match vehicle_store.add_vehicle(vehicle) {
            Ok(_) => {
                self.status_message = Some(("車両を登録しました".to_string(), false));
                // Clear form
                self.new_name.clear();
                self.new_capacity.clear();
                self.new_plate.clear();
                self.new_notes.clear();
                self.new_image_path = None;
            }
            Err(e) => {
                self.status_message = Some((format!("登録エラー: {}", e), true));
            }
        }
    }

    fn render_vehicle_list(&mut self, ui: &mut Ui, vehicle_store: &mut VehicleStore) {
        ui.label(RichText::new("登録済み車両").strong());
        ui.add_space(5.0);

        let vehicles = vehicle_store.all_vehicles();

        if vehicles.is_empty() {
            ui.label(
                RichText::new("登録された車両がありません")
                    .italics()
                    .color(Color32::GRAY),
            );
            return;
        }

        ui.label(format!("{}台登録済み", vehicles.len()));
        ui.add_space(5.0);

        // Collect IDs to delete (to avoid borrow issues)
        let mut to_delete: Option<String> = None;

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                egui::Grid::new("vehicle_list")
                    .num_columns(6)
                    .spacing([10.0, 6.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Header
                        ui.label(RichText::new("車両名").strong());
                        ui.label(RichText::new("積載量").strong());
                        ui.label(RichText::new("クラス").strong());
                        ui.label(RichText::new("ナンバー").strong());
                        ui.label(RichText::new("画像").strong());
                        ui.label("");
                        ui.end_row();

                        for vehicle in vehicles {
                            ui.label(&vehicle.name);
                            ui.label(format!("{:.1}t", vehicle.max_capacity));
                            ui.label(vehicle.truck_class().label());
                            ui.label(vehicle.license_plate.as_deref().unwrap_or("-"));

                            // Image indicator
                            if vehicle.image_path.is_some() {
                                ui.label(RichText::new("✓").color(Color32::LIGHT_GREEN));
                            } else {
                                ui.label(RichText::new("✕").color(Color32::LIGHT_RED));
                            }

                            // Delete button
                            if ui.small_button("削除").clicked() {
                                to_delete = Some(vehicle.id.clone());
                            }
                            ui.end_row();
                        }
                    });
            });

        // Process deletion
        if let Some(id) = to_delete {
            match vehicle_store.remove_vehicle(&id) {
                Ok(true) => {
                    self.status_message = Some(("車両を削除しました".to_string(), false));
                }
                Ok(false) => {
                    self.status_message = Some(("車両が見つかりません".to_string(), true));
                }
                Err(e) => {
                    self.status_message = Some((format!("削除エラー: {}", e), true));
                }
            }
        }
    }
}

impl Default for VehiclePanel {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a base64 thumbnail from image path
fn create_thumbnail(image_path: &str) -> Option<String> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(image_path).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;

    // For now, just encode the whole image as base64
    // In production, you'd want to resize it first
    use base64::{engine::general_purpose::STANDARD, Engine};
    Some(STANDARD.encode(&buffer))
}

/// Scan a vehicle folder for 車検証 and photo images
fn scan_vehicle_folder(folder_path: &PathBuf) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut shaken_images = Vec::new();
    let mut photo_images = Vec::new();

    let image_extensions = ["jpg", "jpeg", "png", "gif", "bmp", "webp"];

    if let Ok(entries) = std::fs::read_dir(folder_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let extension = path.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_default();

            if !image_extensions.contains(&extension.as_str()) {
                continue;
            }

            let filename = path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_lowercase())
                .unwrap_or_default();

            // Detect 車検証 images by filename patterns
            if filename.contains("車検") || filename.contains("shaken")
                || filename.contains("certificate") || filename.contains("registration")
                || filename.contains("検査") || filename.starts_with("cert")
            {
                shaken_images.push(path);
            } else {
                // All other images are considered photos
                photo_images.push(path);
            }
        }
    }

    // Sort by filename
    shaken_images.sort();
    photo_images.sort();

    (shaken_images, photo_images)
}

/// Prompt for extracting vehicle info from 車検証
const SHAKEN_ANALYSIS_PROMPT: &str = r#"この画像は日本の自動車検査証（車検証）です。以下の情報を抽出してください。

抽出する項目:
1. 車名（例: 日野, いすゞ, 三菱ふそう, UD）
2. 型式（例: プロフィア, ギガ, スーパーグレート）
3. 最大積載量（kg単位の数値）
4. 車両番号（ナンバープレート）

以下のJSON形式で回答してください:
{
  "vehicleName": "車名 型式",
  "maxCapacityKg": 10000,
  "licensePlate": "品川 100 あ 1234"
}

注意:
- 最大積載量は必ずkg単位の数値で返してください
- 読み取れない項目はnullとしてください
- 車検証でない画像の場合は全てnullとしてください
"#;

/// Result of 車検証 analysis
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShakenAnalysisResult {
    vehicle_name: Option<String>,
    max_capacity_kg: Option<f64>,
    license_plate: Option<String>,
}

/// Process vehicle folders in background thread
fn process_vehicle_folders(
    folders: Vec<ScannedVehicleFolder>,
    backend: String,
    model: Option<String>,
    status_tx: Sender<ProcessStatus>,
    vehicle_tx: Sender<RegisteredVehicle>,
) {
    let total = folders.len();
    let mut success_count = 0;
    let mut fail_count = 0;

    // Configure AI backend
    let ai_backend = match backend.to_lowercase().as_str() {
        "claude" => Backend::Claude,
        "codex" => Backend::Codex,
        _ => Backend::Gemini,
    };

    for (index, folder) in folders.into_iter().enumerate() {
        let current = index + 1;
        let _ = status_tx.send(ProcessStatus::Processing {
            current,
            total,
            name: folder.folder_name.clone(),
        });

        // Process this folder
        match process_single_vehicle(&folder, ai_backend, &model, &status_tx) {
            Ok(vehicle) => {
                let _ = vehicle_tx.send(vehicle);
                let _ = status_tx.send(ProcessStatus::VehicleCompleted {
                    name: folder.folder_name.clone(),
                    success: true,
                    error: None,
                });
                success_count += 1;
            }
            Err(e) => {
                let _ = status_tx.send(ProcessStatus::VehicleCompleted {
                    name: folder.folder_name.clone(),
                    success: false,
                    error: Some(e.clone()),
                });
                fail_count += 1;
            }
        }
    }

    let _ = status_tx.send(ProcessStatus::Completed {
        success_count,
        fail_count,
    });
}

/// Process a single vehicle folder
fn process_single_vehicle(
    folder: &ScannedVehicleFolder,
    backend: Backend,
    model: &Option<String>,
    status_tx: &Sender<ProcessStatus>,
) -> Result<RegisteredVehicle, String> {
    let _ = status_tx.send(ProcessStatus::AnalyzingShaken {
        name: folder.folder_name.clone(),
    });

    // Analyze 車検証 if available
    let (vehicle_name, max_capacity, license_plate) = if !folder.shaken_images.is_empty() {
        analyze_shaken(&folder.shaken_images[0], backend, model)?
    } else {
        // Use folder name as vehicle name, require manual capacity entry
        (folder.folder_name.clone(), None, None)
    };

    // Require max capacity for registration
    let capacity = max_capacity.ok_or_else(|| {
        "最大積載量を検出できませんでした".to_string()
    })?;

    // Get vehicle image (first photo)
    let image_path = folder.photo_images.first()
        .ok_or_else(|| "車両写真がありません".to_string())?;

    let _ = status_tx.send(ProcessStatus::Registering {
        name: folder.folder_name.clone(),
    });

    // Create thumbnail
    let thumbnail = create_thumbnail(&image_path.display().to_string());

    // Create vehicle
    let mut vehicle = RegisteredVehicle::new(vehicle_name, capacity)
        .with_image(image_path.display().to_string(), thumbnail);

    if let Some(plate) = license_plate {
        vehicle = vehicle.with_license_plate(plate);
    }

    vehicle.notes = Some(format!("フォルダから自動登録: {}", folder.folder_name));

    Ok(vehicle)
}

/// Analyze 車検証 image to extract vehicle information
fn analyze_shaken(
    image_path: &PathBuf,
    backend: Backend,
    model: &Option<String>,
) -> Result<(String, Option<f64>, Option<String>), String> {
    // Configure AI options
    let mut options = if let Some(ref m) = model {
        AnalyzeOptions::with_model(m)
    } else {
        AnalyzeOptions::default()
    };
    options = options.with_backend(backend).json();

    // Call AI
    let response = analyze(SHAKEN_ANALYSIS_PROMPT, &[image_path.clone()], options)
        .map_err(|e| format!("AI解析エラー: {}", e))?;

    // Parse response
    let json_str = extract_json_from_response(&response);
    let result: ShakenAnalysisResult = serde_json::from_str(&json_str)
        .map_err(|e| format!("JSON解析エラー: {}", e))?;

    // Extract vehicle name
    let vehicle_name = result.vehicle_name
        .unwrap_or_else(|| "不明".to_string());

    // Convert kg to tonnes
    let max_capacity = result.max_capacity_kg.map(|kg| kg / 1000.0);

    Ok((vehicle_name, max_capacity, result.license_plate))
}

/// Extract JSON from AI response (handles markdown code blocks)
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
