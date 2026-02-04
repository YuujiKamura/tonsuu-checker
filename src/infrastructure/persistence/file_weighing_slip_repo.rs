//! File-based implementation of WeighingSlipRepository
//!
//! Note: Prepared for storing weighing slip data.
//! Currently unused but maintained for planned weighing slip feature.

#![allow(dead_code)]

use std::path::PathBuf;

use chrono::NaiveDate;

use crate::domain::model::WeighingSlip;
use crate::domain::repository::WeighingSlipRepository;
use crate::error::Error;
use crate::infrastructure::csv_loader::load_weighing_slips;

/// File-based WeighingSlip repository (CSV)
pub struct FileWeighingSlipRepository {
    csv_path: PathBuf,
    slips: Vec<WeighingSlip>,
}

impl FileWeighingSlipRepository {
    /// Create a new repository from a CSV file path
    pub fn new(csv_path: PathBuf) -> Result<Self, Error> {
        let slips =
            load_weighing_slips(&csv_path).map_err(|e| Error::CsvLoader(e.to_string()))?;
        Ok(Self { csv_path, slips })
    }

    /// Get the CSV path
    pub fn csv_path(&self) -> &PathBuf {
        &self.csv_path
    }

    /// Reload data from CSV
    pub fn reload(&mut self) -> Result<(), Error> {
        self.slips =
            load_weighing_slips(&self.csv_path).map_err(|e| Error::CsvLoader(e.to_string()))?;
        Ok(())
    }
}

impl WeighingSlipRepository for FileWeighingSlipRepository {
    fn find_all(&self) -> Result<Vec<WeighingSlip>, Error> {
        Ok(self.slips.clone())
    }

    fn find_by_date(&self, date: NaiveDate) -> Result<Vec<WeighingSlip>, Error> {
        Ok(self
            .slips
            .iter()
            .filter(|s| s.date == date)
            .cloned()
            .collect())
    }

    fn find_by_site(&self, site_name: &str) -> Result<Vec<WeighingSlip>, Error> {
        Ok(self
            .slips
            .iter()
            .filter(|s| s.site_name.contains(site_name))
            .cloned()
            .collect())
    }

    fn find_by_vehicle(&self, vehicle_number: &str) -> Result<Vec<WeighingSlip>, Error> {
        Ok(self
            .slips
            .iter()
            .filter(|s| s.vehicle_number == vehicle_number)
            .cloned()
            .collect())
    }

    fn find_overloaded(&self) -> Result<Vec<WeighingSlip>, Error> {
        Ok(self
            .slips
            .iter()
            .filter(|s| s.is_overloaded)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_csv() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        // Write CP932 encoded CSV content
        let content = "伝票番号,日付,品名,数量(t),累計(t),納入回数,車両番号,運送会社,現場,最大積載量(t),超過\n\
                       S001,2025/11/28,ASガラ,4.04,4.04,1,1122,松尾運搬社,長嶺南6丁目,3.75,超過\n\
                       S002,2025/11/28,ASガラ,3.50,7.54,2,1111,松尾運搬社,長嶺南6丁目,3.5,\n";

        // Encode as CP932
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(content);
        file.write_all(&encoded).unwrap();
        file
    }

    #[test]
    fn test_find_all() {
        let csv = create_test_csv();
        let repo = FileWeighingSlipRepository::new(csv.path().to_path_buf()).unwrap();
        let slips = repo.find_all().unwrap();
        assert_eq!(slips.len(), 2);
    }

    #[test]
    fn test_find_overloaded() {
        let csv = create_test_csv();
        let repo = FileWeighingSlipRepository::new(csv.path().to_path_buf()).unwrap();
        let overloaded = repo.find_overloaded().unwrap();
        assert_eq!(overloaded.len(), 1);
        assert_eq!(overloaded[0].slip_number, "S001");
    }
}
