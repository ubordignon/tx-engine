# Tx-engine

Toy transaction engine that reads transactions and updates user accounts.

## Usage

Invoke as `cargo run -- path/to/transactions.csv > accounts.csv`

Caveats: `transactions.csv` is expected to be formatted according to the
[csv standard](https://datatracker.ietf.org/doc/html/rfc4180). Whitespaces are
filtered out, but missing commas for optional fields, such as the amount field
for "resolve" transactions, will break the parser.

## Considerations

### Transaction resolution

A transaction dispute is considered resolved when the original transaction is determined
to have taken place as stated.

### Transaction chargeback

A chargeback is issues when the disputed transaction was determined _not_ to have
taken place as stated. Specifically:

* Deposit: when a chargeback is issued for a deposit, the client receives their
funds back and the corresponding total account credit is decreased of the same amount.
* Withdrawal: conversely, a chargeback issued on a withdrawal implies that the
user is once more able to use the disputed funds

In either circumstance, the account will be frozen.

### Testing

The test suite is mostly concerned with determining that applying a given
transaction to an account has the intended consequences. Changing transaction
application behavior is meant to break the test.

Correct serde for transactions and accounts is also tested.

### Dealing with inconsistencies

Account creation methods, i.e. `Accounts::from_transactions` and
`Accounts::from_transaction_iter` expose a `strict` parameter. When set to
`false`, a class of common errors encountered during parsing will be disregarded.
Set to `true` to prevent swallowing any error.

## TODOs

* Add async version of `Accounts::from_transaction_iter` to support concurrent
streams of transactions
