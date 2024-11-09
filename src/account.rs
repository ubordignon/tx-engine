use std::{collections::HashMap, fmt::Display, io::stdout};

use serde::{Serialize, Serializer};
use thiserror::Error;

use super::{
    transaction::{Transaction, TransactionType},
    types::{ClientId, TransactionId},
};

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("csv error: {0}")]
    CsvError(#[from] csv::Error),
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("insufficient funds to apply withdrawal, account: {0}, withdrawal: {1}")]
    WithdrawalError(ClientId, TransactionId),
}

type TransactionMap = HashMap<TransactionId, Transaction>;

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

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Account {
    client: ClientId,
    #[serde(skip)]
    transactions: TransactionMap,
    #[serde(serialize_with = "serialize_f64_to_decimal_precision")]
    available: f64,
    #[serde(serialize_with = "serialize_f64_to_decimal_precision")]
    held: f64,
    #[serde(serialize_with = "serialize_f64_to_decimal_precision")]
    total: f64,
    locked: bool,
}

impl Account {
    pub fn new(client: ClientId) -> Self {
        Self {
            client,
            ..Self::default()
        }
    }

    pub fn apply_transaction(&mut self, tx: Transaction) -> Result<(), AccountError> {
        if *tx.client() != self.client {
            panic!(
                "applied transaction on client {} to account {}",
                tx.client(),
                self.client
            );
        }

        self.transactions.insert(*tx.tx(), tx.clone());
        match &tx.type_() {
            TransactionType::Deposit => {
                let amount = tx
                    .amount()
                    .expect("deposits should be some non zero amount");
                self.available += amount;
                self.total += amount;
            }
            TransactionType::Withdrawal => {
                let amount = tx
                    .amount()
                    .expect("withdrawals should be some non zero amount");
                if self.available < amount {
                    return Err(AccountError::WithdrawalError(self.client, *tx.tx()));
                }
                self.available -= amount;
                self.total -= amount;
            }
            TransactionType::Dispute => (),
            TransactionType::Resolve => (),
            TransactionType::Chargeback => (),
        }
        Ok(())
    }
}

impl Display for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Account {} (available: {}, total: {}, locked: {})",
            self.client, self.available, self.total, self.locked,
        )
    }
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
    use super::{Account, AccountError, Accounts, Transaction, TransactionMap, TransactionType};

    #[test]
    fn serialize_accounts() {
        let accounts = Accounts(vec![
            Account {
                client: 1,
                transactions: TransactionMap::default(),
                available: 1.5,
                held: 0.0,
                total: 1.5,
                locked: false,
            },
            Account {
                client: 2,
                transactions: TransactionMap::default(),
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
            transactions: TransactionMap::default(),
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

    #[test]
    #[should_panic(expected = "applied transaction on client 1 to account 0")]
    fn apply_transaction_to_wrong_account() {
        // If a transaction on client x is applied to account y, then it is an implementation issue
        // and it should panic.
        Account::default()
            .apply_transaction(Transaction::new(TransactionType::Deposit, 1, 1, Some(1.0)))
            .unwrap();
    }

    #[test]
    fn apply_deposit() {
        let deposit_amount = 1.0;
        let deposit = Transaction::new(TransactionType::Deposit, 1, 1, Some(deposit_amount));
        let mut account = Account::new(1);
        account.apply_transaction(deposit.clone()).unwrap();
        let mut transactions = TransactionMap::new();
        transactions.insert(*deposit.tx(), deposit);
        assert_eq!(
            account,
            Account {
                client: 1,
                transactions,
                available: deposit_amount,
                held: 0.0,
                total: deposit_amount,
                locked: false
            }
        );
    }

    #[test]
    #[should_panic(expected = "deposits should be some non zero amount")]
    fn apply_deposit_with_none_amount() {
        Account::new(1)
            .apply_transaction(Transaction::new(TransactionType::Deposit, 1, 1, None))
            .unwrap();
    }

    #[test]
    fn apply_withdrawal() {
        let withdrawal_amount = 1.0;
        let withdrawal =
            Transaction::new(TransactionType::Withdrawal, 1, 1, Some(withdrawal_amount));
        let mut account = Account {
            client: 1,
            transactions: TransactionMap::default(),
            available: withdrawal_amount,
            held: 0.0,
            total: withdrawal_amount,
            locked: false,
        };
        account.apply_transaction(withdrawal.clone()).unwrap();
        let mut transactions = TransactionMap::new();
        transactions.insert(*withdrawal.tx(), withdrawal);
        assert_eq!(
            account,
            Account {
                client: 1,
                transactions,
                available: 0.0,
                held: 0.0,
                total: 0.0,
                locked: false
            }
        );
    }

    #[test]
    fn apply_withdrawal_overdrawn() {
        let withdrawal = Transaction::new(TransactionType::Withdrawal, 1, 1, Some(1.0));
        let mut account = Account {
            client: 1,
            transactions: TransactionMap::default(),
            available: 0.0,
            held: 0.0,
            total: 0.0,
            locked: false,
        };
        assert!(matches!(
            account.apply_transaction(withdrawal.clone()).unwrap_err(),
            AccountError::WithdrawalError(1, 1)
        ));
    }

    #[test]
    #[should_panic(expected = "withdrawals should be some non zero amount")]
    fn apply_withdrawal_with_none_amount() {
        Account::new(1)
            .apply_transaction(Transaction::new(TransactionType::Withdrawal, 1, 1, None))
            .unwrap();
    }
}
