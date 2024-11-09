use tx_engine::{Accounts, TransactionsCsv};

use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let transactions = env::args()
        .nth(1)
        .expect("provide a csv file with transactions to parse");
    let mut transactions = TransactionsCsv::from_csv(&transactions)?;

    let accounts = Accounts::from_transaction_iter(transactions.iter(), false)?;
    accounts.to_csv()?;

    Ok(())
}
