use std::{fmt::Display, fs::File};

use csv::{DeserializeRecordsIter, Error as CsvError, Reader as CsvReader};
use derive_getters::Getters;
use derive_more::{Constructor, Deref, DerefMut};
use serde::Deserialize;
use thiserror::Error;

use super::types::{ClientId, TransactionId};

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Clone, Constructor, Debug, Deserialize, Getters, PartialEq)]
pub struct Transaction {
    #[serde(rename = "type")]
    type_: TransactionType,
    client: ClientId,
    tx: TransactionId,
    amount: Option<f64>,
    #[serde(skip)]
    disputed: bool,
}

impl Transaction {
    pub fn dispute(&mut self) {
        self.disputed = true;
    }

    pub fn resolve(&mut self) {
        self.disputed = false;
    }
}
impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(amount) = self.amount {
            write!(
                f,
                "Transaction {} (type: {:?}, client: {}, amount: {:?})",
                self.tx, self.type_, self.client, amount,
            )
        } else {
            write!(
                f,
                "Transaction {} (type: {:?}, client: {})",
                self.tx, self.type_, self.client,
            )
        }
    }
}

#[derive(Debug, Default, Deref, DerefMut, PartialEq)]
pub struct Transactions(pub Vec<Transaction>);

impl Transactions {
    pub fn from_csv(path: &str) -> Result<Self, CsvError> {
        CsvReader::from_path(path)?
            .deserialize()
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("csv error: {0}")]
    Csv(#[from] CsvError),
}

pub struct TransactionsCsv(CsvReader<File>);

impl TransactionsCsv {
    pub fn from_csv(path: &str) -> Result<Self, CsvError> {
        Ok(Self(CsvReader::from_path(path)?))
    }
}

impl TransactionsCsv {
    pub fn iter(&mut self) -> TransactionCsvIterator<'_> {
        TransactionCsvIterator {
            csv_deserializer: self.0.deserialize(),
        }
    }
}

pub struct TransactionCsvIterator<'a> {
    csv_deserializer: DeserializeRecordsIter<'a, File, Transaction>,
}

impl Iterator for TransactionCsvIterator<'_> {
    type Item = Result<Transaction, TransactionError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.csv_deserializer
            .next()
            .map(|tx| tx.map_err(|e| e.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::{Transaction, TransactionType, Transactions, TransactionsCsv};

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
                    amount: Some(2.0),
                    disputed: false
                },
                Transaction {
                    type_: TransactionType::Withdrawal,
                    client: 1,
                    tx: 2,
                    amount: Some(1.5),
                    disputed: false
                },
                Transaction {
                    type_: TransactionType::Dispute,
                    client: 1,
                    tx: 2,
                    amount: None,
                    disputed: false
                },
                Transaction {
                    type_: TransactionType::Resolve,
                    client: 1,
                    tx: 2,
                    amount: None,
                    disputed: false
                },
                Transaction {
                    type_: TransactionType::Chargeback,
                    client: 1,
                    tx: 2,
                    amount: None,
                    disputed: false
                },
            ])
        );
    }

    #[test]
    fn deserialize_transactions_iterator() {
        let sample_path = "src/test_utils/test_txs.csv";
        let mut transactions_csv = TransactionsCsv::from_csv(sample_path).unwrap();
        let transactions = transactions_csv
            .iter()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(transactions, Transactions::from_csv(sample_path).unwrap().0);
    }
}
