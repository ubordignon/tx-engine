use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct Transaction {
    #[serde(rename = "type")]
    type_: TransactionType,
    client: u16,
    tx: u32,
    amount: Option<f64>,
}

#[derive(Debug, PartialEq)]
pub struct Transactions(pub Vec<Transaction>);

impl Transactions {
    pub fn from_csv(path: &str) -> Result<Self, csv::Error> {
        csv::Reader::from_path(path)?
            .deserialize()
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::{Transaction, TransactionType, Transactions};

    #[test]
    fn deserialize_transactions() {
        let sample_path = "src/test_utils/test_txs.csv";
        let transactions = Transactions::from_csv(sample_path).unwrap();
        assert_eq!(
            transactions,
            Transactions(vec![
                Transaction {
                    type_: TransactionType::Deposit,
                    client: 1,
                    tx: 1,
                    amount: Some(2.0)
                },
                Transaction {
                    type_: TransactionType::Withdrawal,
                    client: 1,
                    tx: 2,
                    amount: Some(1.5)
                },
                Transaction {
                    type_: TransactionType::Dispute,
                    client: 1,
                    tx: 2,
                    amount: None
                },
                Transaction {
                    type_: TransactionType::Resolve,
                    client: 1,
                    tx: 2,
                    amount: None
                },
                Transaction {
                    type_: TransactionType::Chargeback,
                    client: 1,
                    tx: 2,
                    amount: None
                },
            ])
        );
    }
}
