use std::{collections::HashMap, fmt::Display, io::stdout};

use derive_more::{Deref, DerefMut};
use serde::{Serialize, Serializer};
use thiserror::Error;

use super::{
    transaction::{Transaction, TransactionError, TransactionType, Transactions},
    types::{ClientId, TransactionId},
};

#[derive(Debug, Error)]
pub enum AccountError {
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("insufficient funds to apply withdrawal, account: {0}, withdrawal: {1}")]
    Withdrawal(ClientId, TransactionId),
    #[error("disputed transaction not found, account, {0}, transaction: {1}")]
    Dispute(ClientId, TransactionId),
    #[error("resolved transaction not found, account, {0}, transaction: {1}")]
    Resolve(ClientId, TransactionId),
    #[error("resolved transaction wasn't disputed, account, {0}, transaction: {1}")]
    ResolveUndisputed(ClientId, TransactionId),
    #[error("chargeback transaction not found, account, {0}, transaction: {1}")]
    Chargeback(ClientId, TransactionId),
    #[error("chargeback transaction wasn't disputed, account, {0}, transaction: {1}")]
    ChargebackUndisputed(ClientId, TransactionId),
    #[error("transaction error: {0}")]
    Transaction(#[from] TransactionError),
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

    fn freeze(&mut self) {
        self.locked = true;
    }

    pub fn apply_transaction(&mut self, tx: Transaction) -> Result<(), AccountError> {
        if *tx.client() != self.client {
            panic!(
                "applied transaction on client {} to account {}",
                tx.client(),
                self.client
            );
        }

        match &tx.type_() {
            TransactionType::Deposit => {
                let amount = tx.amount();
                self.available += amount;
                self.total += amount;
                if let Some(tx_clashed) = self.transactions.insert(*tx.tx(), tx) {
                    panic!(
                        "multiple transactions with the same id: {}",
                        *tx_clashed.tx()
                    );
                }
            }
            TransactionType::Withdrawal => {
                let amount = tx.amount();
                if self.available < amount {
                    return Err(AccountError::Withdrawal(self.client, *tx.tx()));
                }
                self.available -= amount;
                self.total -= amount;
                if let Some(tx_clashed) = self.transactions.insert(*tx.tx(), tx) {
                    panic!(
                        "multiple transactions with the same id: {}",
                        *tx_clashed.tx()
                    );
                }
            }
            TransactionType::Dispute => {
                let disputed = self
                    .transactions
                    .get_mut(tx.tx())
                    .ok_or(AccountError::Dispute(self.client, *tx.tx()))?;
                match disputed.type_() {
                    TransactionType::Deposit => {
                        let amount = disputed.amount();
                        self.available -= amount;
                        self.held += amount;
                        disputed.dispute();
                    }
                    TransactionType::Withdrawal => {
                        // Disputing a withdrawal, e.g. disputing having received amount withdrawn.
                        // A valid withdrawal dispute would imply that the client has once more a
                        // total amount of funds that includes the ones they attempted to withdraw.
                        let amount = disputed.amount();
                        self.held += amount;
                        self.total += amount;
                        disputed.dispute();
                    }
                    _ => panic!("deposits and withdrawals are the only transaction types stored"),
                }
            }
            TransactionType::Resolve => {
                let disputed = self
                    .transactions
                    .get_mut(tx.tx())
                    .ok_or(AccountError::Resolve(self.client, *tx.tx()))?;
                if !disputed.disputed() {
                    return Err(AccountError::ResolveUndisputed(self.client, *disputed.tx()));
                }
                match disputed.type_() {
                    TransactionType::Deposit => {
                        let amount = disputed.amount();
                        self.available += amount;
                        self.held -= amount;
                    }
                    TransactionType::Withdrawal => {
                        // The withdrawal dispute was resolved, which means e.g. that the dispute
                        // claim was withdrawn, pun unintended. In other words, the withdrawal took
                        // place as expected and the funds involved cannot be credited to the
                        // client any longer.
                        let amount = disputed.amount();
                        self.held -= amount;
                        self.total -= amount;
                    }
                    _ => panic!("deposits and withdrawals are the only transaction types stored"),
                }
                disputed.resolve();
            }
            TransactionType::Chargeback => {
                let disputed = self
                    .transactions
                    .get_mut(tx.tx())
                    .ok_or(AccountError::Chargeback(self.client, *tx.tx()))?;
                if !disputed.disputed() {
                    return Err(AccountError::ChargebackUndisputed(
                        self.client,
                        *disputed.tx(),
                    ));
                }
                match disputed.type_() {
                    TransactionType::Deposit => {
                        let amount = disputed.amount();
                        self.held -= amount;
                        self.total -= amount;
                        disputed.resolve();
                    }
                    TransactionType::Withdrawal => {
                        // If a chargeback was issued for a withdrawal transaction, then the
                        // withdrawal didn't take place as expected, and those funds should once
                        // more become available to the client.
                        let amount = disputed.amount();
                        self.available += amount;
                        self.held -= amount;
                    }
                    _ => panic!("deposits and withdrawals are the only transaction types stored"),
                }
                disputed.resolve();
                self.freeze();
            }
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

#[derive(Default, Deref, DerefMut)]
pub struct Accounts(HashMap<ClientId, Account>);

impl Accounts {
    pub fn from_transaction_iter<T: Iterator<Item = Result<Transaction, TransactionError>>>(
        tx_iter: T,
        strict: bool,
    ) -> Result<Self, AccountError> {
        let mut accounts = Self::default();
        for tx in tx_iter {
            let tx = tx?;
            if let Err(e) = accounts
                .entry(*tx.client())
                .or_insert(Account::new(*tx.client()))
                .apply_transaction(tx)
            {
                if !strict
                    && matches!(
                        e,
                        AccountError::Withdrawal(..)
                            | AccountError::Dispute(..)
                            | AccountError::Resolve(..)
                            | AccountError::ResolveUndisputed(..)
                            | AccountError::Chargeback(..)
                            | AccountError::ChargebackUndisputed(..)
                    )
                {
                    continue;
                }
                return Err(e);
            }
        }
        Ok(accounts)
    }

    pub fn from_transactions(
        transactions: Transactions,
        strict: bool,
    ) -> Result<Self, AccountError> {
        Self::from_transaction_iter(transactions.0.into_iter().map(Ok), strict)
    }

    pub fn to_csv(&self) -> Result<(), AccountError> {
        let mut wrt = csv::Writer::from_writer(stdout());
        for acc in self.0.values() {
            wrt.serialize(acc)?;
        }
        wrt.flush()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Account, AccountError, Transaction, TransactionMap, TransactionType};

    #[test]
    fn serialize_accounts() {
        let accounts = vec![
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
        ];

        let mut wrt = csv::Writer::from_writer(vec![]);
        for acc in accounts {
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
            .apply_transaction(Transaction::new(
                TransactionType::Deposit,
                1,
                1,
                Some(1.0),
                false,
            ))
            .unwrap();
    }

    #[test]
    fn apply_deposit() {
        let deposit_amount = 1.0;
        let deposit = Transaction::new(TransactionType::Deposit, 1, 1, Some(deposit_amount), false);
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
                total: deposit_amount,
                ..Account::default()
            }
        );
    }

    #[test]
    fn apply_withdrawal() {
        let withdrawal_amount = 1.0;
        let withdrawal = Transaction::new(
            TransactionType::Withdrawal,
            1,
            1,
            Some(withdrawal_amount),
            false,
        );
        let mut account = Account {
            client: 1,
            transactions: TransactionMap::default(),
            available: withdrawal_amount,
            total: withdrawal_amount,
            ..Account::default()
        };
        account.apply_transaction(withdrawal.clone()).unwrap();
        let mut transactions = TransactionMap::new();
        transactions.insert(*withdrawal.tx(), withdrawal);
        assert_eq!(
            account,
            Account {
                client: 1,
                transactions,
                ..Account::default()
            }
        );
    }

    #[test]
    fn apply_withdrawal_overdrawn() {
        let withdrawal = Transaction::new(TransactionType::Withdrawal, 1, 1, Some(1.0), false);
        let mut account = Account {
            client: 1,
            transactions: TransactionMap::default(),
            ..Account::default()
        };
        assert!(matches!(
            account.apply_transaction(withdrawal.clone()).unwrap_err(),
            AccountError::Withdrawal(1, 1)
        ));
    }

    #[test]
    fn apply_dispute() {
        let available = 9.0;
        let held = 0.0;
        let total = available;
        let mut account = Account {
            client: 1,
            transactions: TransactionMap::default(),
            available,
            held,
            total,
            locked: false,
        };

        let tx_amount = 1.0;
        let deposit = Transaction::new(TransactionType::Deposit, 1, 1, Some(tx_amount), false);
        // Increase available and total by `tx_amount`
        account.apply_transaction(deposit.clone()).unwrap();
        // Decrease available and increase held by `tx_amount`
        account
            .apply_transaction(Transaction::new(
                TransactionType::Dispute,
                1,
                1,
                None,
                false,
            ))
            .unwrap();

        // Available was increased and decreased by the same amount
        assert_eq!(account.available, available);
        assert_eq!(account.held, tx_amount);
        assert_eq!(account.total, total + tx_amount);
        assert_eq!(account.total, account.available + account.held);
        assert!(*account.transactions.get(&1).unwrap().disputed());

        let withdrawal =
            Transaction::new(TransactionType::Withdrawal, 1, 2, Some(tx_amount), false);
        // Decrease available and total by `tx_amount`
        account.apply_transaction(withdrawal.clone()).unwrap();
        // Increase held and total by `tx_amount`
        account
            .apply_transaction(Transaction::new(
                TransactionType::Dispute,
                1,
                2,
                None,
                false,
            ))
            .unwrap();

        // Available is not restored by withdrawal dispute
        assert_eq!(account.available, available - tx_amount);
        assert_eq!(account.held, tx_amount * 2.0);
        // Total is not changed, as a result of the dispute
        assert_eq!(account.total, total + tx_amount);
        assert_eq!(account.total, account.available + account.held);
        assert!(*account.transactions.get(&2).unwrap().disputed());

        assert!(matches!(
            account
                .apply_transaction(Transaction::new(
                    TransactionType::Dispute,
                    1,
                    3,
                    None,
                    false,
                ))
                .unwrap_err(),
            AccountError::Dispute(1, 3)
        ));
    }

    #[test]
    fn apply_resolve() {
        let available = 8.0;
        let held = 2.0;
        let total = available + held;

        let tx_amount = 1.0;
        let deposit = Transaction::new(TransactionType::Deposit, 1, 1, Some(tx_amount), true);
        let withdrawal = Transaction::new(TransactionType::Withdrawal, 1, 2, Some(tx_amount), true);
        let mut transactions = TransactionMap::new();
        transactions.insert(*deposit.tx(), deposit);
        transactions.insert(*withdrawal.tx(), withdrawal);

        let mut account = Account {
            client: 1,
            transactions,
            available,
            held,
            total,
            locked: false,
        };

        account
            .apply_transaction(Transaction::new(
                TransactionType::Resolve,
                1,
                1,
                None,
                false,
            ))
            .unwrap();

        assert_eq!(account.available, available + tx_amount);
        assert_eq!(account.held, held - tx_amount);
        assert_eq!(account.total, total);
        assert_eq!(account.total, account.available + account.held);
        assert!(!*account.transactions.get(&1).unwrap().disputed());

        account
            .apply_transaction(Transaction::new(
                TransactionType::Resolve,
                1,
                2,
                None,
                false,
            ))
            .unwrap();

        assert_eq!(account.available, available + tx_amount);
        assert_eq!(account.held, held - tx_amount * 2.0);
        assert_eq!(account.total, total - tx_amount);
        assert_eq!(account.total, account.available + account.held);
        assert!(!*account.transactions.get(&2).unwrap().disputed());

        assert!(matches!(
            account
                .apply_transaction(Transaction::new(
                    TransactionType::Resolve,
                    1,
                    3,
                    None,
                    false,
                ))
                .unwrap_err(),
            AccountError::Resolve(1, 3)
        ));

        account
            .apply_transaction(Transaction::new(
                TransactionType::Withdrawal,
                1,
                3,
                Some(tx_amount),
                false,
            ))
            .unwrap();

        assert!(matches!(
            account
                .apply_transaction(Transaction::new(
                    TransactionType::Resolve,
                    1,
                    3,
                    None,
                    false,
                ))
                .unwrap_err(),
            AccountError::ResolveUndisputed(1, 3)
        ));
    }

    #[test]
    fn apply_chargeback() {
        let available = 8.0;
        let held = 2.0;
        let total = available + held;

        let tx_amount = 1.0;
        let deposit = Transaction::new(TransactionType::Deposit, 1, 1, Some(tx_amount), true);
        let withdrawal = Transaction::new(TransactionType::Withdrawal, 1, 2, Some(tx_amount), true);
        let mut transactions = TransactionMap::new();
        transactions.insert(*deposit.tx(), deposit);
        transactions.insert(*withdrawal.tx(), withdrawal);

        let mut account = Account {
            client: 1,
            transactions,
            available,
            held,
            total,
            locked: false,
        };

        account
            .apply_transaction(Transaction::new(
                TransactionType::Chargeback,
                1,
                1,
                None,
                false,
            ))
            .unwrap();

        assert_eq!(account.available, available);
        assert_eq!(account.held, held - tx_amount);
        assert_eq!(account.total, total - tx_amount);
        assert_eq!(account.total, account.available + account.held);
        assert!(!*account.transactions.get(&1).unwrap().disputed());
        assert!(account.locked);

        account.locked = false;
        account
            .apply_transaction(Transaction::new(
                TransactionType::Chargeback,
                1,
                2,
                None,
                false,
            ))
            .unwrap();

        assert_eq!(account.available, available + tx_amount);
        assert_eq!(account.held, held - tx_amount * 2.0);
        assert_eq!(account.total, total - tx_amount);
        assert_eq!(account.total, account.available + account.held);
        assert!(!*account.transactions.get(&2).unwrap().disputed());

        assert!(matches!(
            account
                .apply_transaction(Transaction::new(
                    TransactionType::Chargeback,
                    1,
                    3,
                    None,
                    false,
                ))
                .unwrap_err(),
            AccountError::Chargeback(1, 3)
        ));

        account
            .apply_transaction(Transaction::new(
                TransactionType::Withdrawal,
                1,
                3,
                Some(tx_amount),
                false,
            ))
            .unwrap();

        assert!(matches!(
            account
                .apply_transaction(Transaction::new(
                    TransactionType::Chargeback,
                    1,
                    3,
                    None,
                    false,
                ))
                .unwrap_err(),
            AccountError::ChargebackUndisputed(1, 3)
        ));
    }
}
