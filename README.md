# ecdsa-dump-bitcoin

[![Rust 1.44+](https://img.shields.io/badge/rust-1.44+-red.svg)](https://www.sagemath.org/index.html) [![License: GPL v3](https://img.shields.io/badge/license-GPL%20v3-blue.svg)](http://www.gnu.org/licenses/gpl-3.0)

**Note**: This is a fork of [rusty-blockparser](https://github.com/gcarq/rusty-blockparser)

Dump bitcoin signatures and original messages

## Changes from upstream

* Added `sigdump` callback

## Building

All you need is Rust, which can be installed using [rustup](https://rustup.rs/).


```bash
cargo build --release
```

It is important to build with `--release`, for better performance!


## Usage
```
USAGE:
    rusty-blockparser [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
    -v               Increases verbosity level. Info=0, Debug=1, Trace=2 (default: 0)
        --verify     Verifies the leveldb index integrity and verifies merkle roots

OPTIONS:
    -d, --blockchain-dir <blockchain-dir>    Sets blockchain directory which contains blk.dat files (default:
                                             ~/.bitcoin/blocks)
    -c, --coin <NAME>                        Specify blockchain coin (default: bitcoin) [possible values: bitcoin,
                                             testnet3, namecoin, litecoin, dogecoin, myriadcoin, unobtanium]
    -e, --end <NUMBER>                       Specify last block for parsing (inclusive) (default: all known blocks)
    -s, --start <NUMBER>                     Specify starting block for parsing (inclusive)

SUBCOMMANDS:
    balances          Dumps all addresses with non-zero balance to CSV file
    csvdump           Dumps the whole blockchain into CSV files
    help              Prints this message or the help of the given subcommand(s)
    sigdump           Dumps signatures to CSV file
    simplestats       Shows various Blockchain stats
    unspentcsvdump    Dumps the unspent outputs to CSV file
```
### Example

To dump all ecdsa signatures and original messages from the Bitcoin chain, 
do the following.

First install Bitcoin core and run it with transaction indexing enabled:

```
bitcoin-qt -txindex=1
```

During the first run, make sure to note where the bitcoin folder is.
By default, it will be in `~/.bitcoin`.
Make sure to disable chain pruning when asked. There may be a checkbox to disable on first run of `bitcoin-qt`.

To dump signatures and messages that were synced so far, 
using the bitcoin folder and to dump them in the dump folder, 
use the `sigdump` callback as in the following example:

```
$ cargo run --release -- sigdump ./dump-folder ~/.bitcoin
[8:41:53 UTC] INFO - main: Starting rusty-blockparser v0.8.1 ...
[8:41:53 UTC] INFO - index: Reading index from /home/nils/.bitcoin/blocks/index ...
[8:41:53 UTC] INFO - index: Got longest chain with 1 blocks ...
[8:41:53 UTC] INFO - blkfile: Reading files from /home/nils/.bitcoin/blocks ...
[8:41:53 UTC] INFO - parser: Parsing Bitcoin blockchain (range=0..) ...
[8:41:53 UTC] INFO - callback: Using `sigdump` with dump folder: ./dump-folder ...
[8:41:53 UTC] INFO - parser: Done. Processed 1 blocks in 0.00 minutes. (avg:     1 blocks/sec)
[8:41:53 UTC] INFO - callback: Done.
Dumped all 1 blocks:
        -> transactions:         1
        -> inputs:               1
        -> outputs:              1
[8:41:53 UTC] INFO - main: Fin.

```

A CSV file will be created in the dump folder.
This output file will contain, on each line:

```
r;s;pubkey;txid;message_hash;block_time
```