use std::io::stdout;

use serde::{Serialize, Serializer};
use thiserror::Error;

use super::transaction::Transactions;

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("csv error: {0}")]
    CsvError(#[from] csv::Error),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
}

const DECIMAL_PRECISION: i32 = 4;

fn serialize_f64_to_decimal_precision<S>(num: &f64, ser: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let (int, mut frac) = (num.trunc(), num.fract());
    frac *= 10.0f64.powi(DECIMAL_PRECISION);
    frac = frac.trunc();
    frac /= 10.0f64.powi(DECIMAL_PRECISION);

    ser.serialize_f64(int + frac)
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Account {
    client: u16,
    #[serde(skip)]
    transactions: Transactions,
    #[serde(serialize_with = "serialize_f64_to_decimal_precision")]
    available: f64,
    #[serde(serialize_with = "serialize_f64_to_decimal_precision")]
    held: f64,
    #[serde(serialize_with = "serialize_f64_to_decimal_precision")]
    total: f64,
    locked: bool,
}

pub struct Accounts(Vec<Account>);

impl Accounts {
    pub fn to_csv(&self) -> Result<(), AccountError> {
        let mut wrt = csv::Writer::from_writer(stdout());
        for acc in &self.0 {
            wrt.serialize(acc)?;
        }
        wrt.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Account, Accounts, Transactions};

    #[test]
    fn serialize_accounts() {
        let accounts = Accounts(vec![
            Account {
                client: 1,
                transactions: Transactions(vec![]),
                available: 1.5,
                held: 0.0,
                total: 1.5,
                locked: false,
            },
            Account {
                client: 2,
                transactions: Transactions(vec![]),
                available: 2.0,
                held: 0.0,
                total: 2.0,
                locked: false,
            },
        ]);

        let mut wrt = csv::Writer::from_writer(vec![]);
        for acc in accounts.0 {
            wrt.serialize(acc).unwrap();
        }
        let accounts = &wrt.into_inner().unwrap();
        let accounts = std::str::from_utf8(accounts).unwrap();
        let accounts_expected = "\
client,available,held,total,locked
1,1.5,0.0,1.5,false
2,2.0,0.0,2.0,false
";
        assert_eq!(accounts, accounts_expected);
    }

    #[test]
    fn serialize_long_floats() {
        let account = Account {
            client: 1,
            transactions: Transactions(vec![]),
            available: 1.11223344,
            held: 0.0,
            total: 1.11223344,
            locked: false,
        };

        let mut wrt = csv::Writer::from_writer(vec![]);
        wrt.serialize(account).unwrap();

        let account = &wrt.into_inner().unwrap();
        let account = std::str::from_utf8(account).unwrap();
        let account_expected = "client,available,held,total,locked\n1,1.1122,0.0,1.1122,false\n";
        assert_eq!(account, account_expected);
    }
}
