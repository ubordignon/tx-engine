use tx_engine::{Accounts, Transactions};

use std::{env, error::Error};

fn main() -> Result<(), Box<dyn Error>> {
    let transactions = env::args()
        .nth(1)
        .expect("provide a csv file with transactions to parse");
    let transactions = Transactions::from_csv(&transactions)?;

    let accounts = Accounts::from_transactions(transactions, false)?;
    accounts.to_csv()?;

    Ok(())
}
