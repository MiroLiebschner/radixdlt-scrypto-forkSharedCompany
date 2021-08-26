use clap::{crate_version, App, ArgMatches, SubCommand};
use radix_engine::execution::*;
use radix_engine::model::*;
use scrypto::buffer::*;
use scrypto::rust::collections::*;
use scrypto::types::*;
use scrypto::utils::*;
use uuid::Uuid;

use crate::cli::*;
use crate::ledger::*;

/// Constructs a `new-account` subcommand.
pub fn make_new_account_cmd<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name(CMD_NEW_ACCOUNT)
        .about("Creates an account")
        .version(crate_version!())
}

/// Handles a `new-account` request.
pub fn handle_new_account<'a>(_matches: &ArgMatches<'a>) {
    let tx_hash = sha256(Uuid::new_v4().to_string());
    let mut ledger = FileBasedLedger::new(get_data_dir());
    let mut runtime = Runtime::new(tx_hash, &mut ledger);

    // create XRD native token
    if runtime.get_resource(Address::RadixToken).is_none() {
        let xrd = Resource {
            symbol: "xrd".to_owned(),
            name: "Radix".to_owned(),
            description: "The Radix Public Network's native token, used to pay the network's required transaction fees and to secure the network through staking to its validator nodes.".to_owned(),
            url: "https://tokens.radixdlt.com".to_owned(),
            icon_url: "https://assets.radixdlt.com/icons/icon-xrd-32x32.png".to_owned(),
            minter: Some(Address::System),
            supply: None,
        };
        runtime.put_resource(Address::RadixToken, xrd);
    }

    // publish smart account blueprint
    let package = Address::Package([0u8; 26]);
    if runtime.get_package(package).is_none() {
        runtime.put_package(
            package,
            Package::new(include_bytes!("account.wasm").to_vec()),
        );
    }
    // create new account
    let mut process = Process::new(0, false, &mut runtime);
    let output = process
        .target_function(package, "Account", "new".to_owned(), Vec::new())
        .and_then(|target| process.run(target))
        .unwrap();
    process.finalize().unwrap();
    let component: scrypto::constructs::Component = scrypto_decode(&output).unwrap();

    // allocate free XRD
    let mut buckets = HashMap::new();
    let bid = runtime.new_transient_bid();
    let bucket = Bucket::new(1_000_000.into(), Address::RadixToken);
    buckets.insert(bid, bucket);

    // deposit
    let mut process2 = Process::new(0, true, &mut runtime);
    process2.put_resources(buckets, HashMap::new());
    process2
        .target_method(
            component.address(),
            "deposit_tokens".to_owned(),
            vec![scrypto_encode(&scrypto::resource::Tokens::from(bid))],
        )
        .and_then(|target| process2.run(target))
        .unwrap();
    process2.finalize().unwrap();

    // flush
    runtime.flush();

    println!("New account: {}", component.address());
}