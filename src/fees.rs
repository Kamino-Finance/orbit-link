use crate::Result;

pub mod solanacompass {
    use super::*;
    use crate::errors::ErrorKind;
    use serde::Deserialize;
    use std::collections::HashMap;

    #[derive(Deserialize, Debug)]
    struct Entry {
        name: String,
        data: HashMap<String, f64>,
    }

    /// Propose a recommended fee based on the fees in the last 5 minutes
    ///
    /// Fee is in microlamports per CU
    pub async fn get_last_5_min_median_fee() -> Result<u64> {
        let url = "https://solanacompass.com/statistics/priorityFees?type=avg";
        let resp = reqwest::get(url).await?.json::<Vec<Entry>>().await?;

        let entry = resp.get(1).ok_or(ErrorKind::SolanaCompassReturnInvalid)?;
        if &entry.name != "Median Fee (60s Avg)" {
            return Err(ErrorKind::SolanaCompassReturnInvalid);
        }
        let mean_fees_data = &entry.data;
        let fees = [
            mean_fees_data.get("2 mins ago"),
            mean_fees_data.get("3 mins ago"),
            mean_fees_data.get("4 mins ago"),
            mean_fees_data.get("5 mins ago"),
        ];
        let fees: Vec<f64> = fees.into_iter().flatten().copied().collect();
        let num_fees = fees.len();
        if num_fees == 0 {
            return Err(ErrorKind::SolanaCompassReturnInvalid);
        }
        let avg_median_fee = fees.into_iter().sum::<f64>() / num_fees as f64;
        // Fee is in SOL per tx (assuming 200_000 CU), convert to micro lamports per CU
        let fee = (avg_median_fee * (10.0_f64.powi(9)) * 1_000_000.0 / 200_000.0) as u64;

        Ok(fee)
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[tokio::test]
        async fn test_get_last_5_min_median_fee() {
            let fee = get_last_5_min_median_fee().await.unwrap();
            println!("fee: {}", fee);
            assert!(fee > 0);
        }
    }
}
