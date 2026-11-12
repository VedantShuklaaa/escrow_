use anchor_lang::{
    prelude::Pubkey, solana_program::instruction::Instruction, AccountDeserialize, InstructionData,
    ToAccountMetas,
};
use escrow::state::Escrow;
use litesvm::LiteSVM;
use solana_account::Account;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

const DECIMALS: u8 = 6;
const SEED: u64 = 1;
const RECEIVE: u64 = 100;
const DEPOSIT: u64 = 400;
const MAKER_A: u64 = 1_000;
const TAKER_B: u64 = 500;

struct Ctx {
    svm: LiteSVM,
    maker: Keypair,
    taker: Keypair,
    mint_a: Pubkey,
    mint_b: Pubkey,
    maker_ata_a: Pubkey,
    taker_ata_b: Pubkey,
    escrow: Pubkey,
    vault: Pubkey,
}

fn ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    anchor_spl::associated_token::get_associated_token_address(owner, mint)
}

fn put_mint(svm: &mut LiteSVM, key: Pubkey) {
    let mut data = vec![0u8; 82];
    data[44] = DECIMALS;
    data[45] = 1;
    svm.set_account(
        key,
        Account {
            lamports: svm.minimum_balance_for_rent_exemption(data.len()),
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn put_token_account(svm: &mut LiteSVM, key: Pubkey, mint: &Pubkey, owner: &Pubkey, amount: u64) {
    let mut data = vec![0u8; 165];
    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());
    data[108] = 1;
    svm.set_account(
        key,
        Account {
            lamports: svm.minimum_balance_for_rent_exemption(data.len()),
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn balance(svm: &LiteSVM, key: &Pubkey) -> u64 {
    let acc = svm.get_account(key).unwrap();
    u64::from_le_bytes(acc.data[64..72].try_into().unwrap())
}

fn is_closed(svm: &LiteSVM, key: &Pubkey) -> bool {
    match svm.get_account(key) {
        None => true,
        Some(acc) => acc.lamports == 0 || acc.data.is_empty(),
    }
}

fn read_escrow(svm: &LiteSVM, key: &Pubkey) -> Escrow {
    let acc = svm.get_account(key).unwrap();
    Escrow::try_deserialize(&mut acc.data.as_slice()).unwrap()
}

fn send(svm: &mut LiteSVM, ix: Instruction, payer: &Keypair) -> Result<(), String> {
    svm.expire_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

fn setup() -> Ctx {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(escrow::ID, "../../target/deploy/escrow.so")
        .unwrap();

    let maker = Keypair::new();
    let taker = Keypair::new();
    svm.airdrop(&maker.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&taker.pubkey(), 10_000_000_000).unwrap();

    let mint_a = Pubkey::new_unique();
    let mint_b = Pubkey::new_unique();
    put_mint(&mut svm, mint_a);
    put_mint(&mut svm, mint_b);

    let maker_ata_a = ata(&maker.pubkey(), &mint_a);
    let taker_ata_b = ata(&taker.pubkey(), &mint_b);
    put_token_account(&mut svm, maker_ata_a, &mint_a, &maker.pubkey(), MAKER_A);
    put_token_account(&mut svm, taker_ata_b, &mint_b, &taker.pubkey(), TAKER_B);

    let (escrow, _) = Pubkey::find_program_address(
        &[b"escrow", maker.pubkey().as_ref(), &SEED.to_le_bytes()],
        &escrow::ID,
    );
    let vault = ata(&escrow, &mint_a);

    Ctx {
        svm,
        maker,
        taker,
        mint_a,
        mint_b,
        maker_ata_a,
        taker_ata_b,
        escrow,
        vault,
    }
}

fn initialize_ix(c: &Ctx, receive: u64) -> Instruction {
    Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::Initialize {
            maker: c.maker.pubkey(),
            mint_a: c.mint_a,
            mint_b: c.mint_b,
            escrow: c.escrow,
            vault: c.vault,
            associated_token_program: anchor_spl::associated_token::ID,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: escrow::instruction::Initialize {
            seed: SEED,
            receive,
        }
        .data(),
    }
}

fn make_ix(c: &Ctx, deposit: u64) -> Instruction {
    Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::Make {
            maker: c.maker.pubkey(),
            mint_a: c.mint_a,
            maker_ata_a: c.maker_ata_a,
            escrow: c.escrow,
            vault: c.vault,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: escrow::instruction::Make { deposit }.data(),
    }
}

fn take_ix(c: &Ctx) -> Instruction {
    Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::Take {
            taker: c.taker.pubkey(),
            maker: c.maker.pubkey(),
            mint_a: c.mint_a,
            mint_b: c.mint_b,
            taker_ata_a: ata(&c.taker.pubkey(), &c.mint_a),
            taker_ata_b: c.taker_ata_b,
            maker_ata_b: ata(&c.maker.pubkey(), &c.mint_b),
            escrow: c.escrow,
            vault: c.vault,
            associated_token_program: anchor_spl::associated_token::ID,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: escrow::instruction::Take {}.data(),
    }
}

fn refund_ix(c: &Ctx) -> Instruction {
    Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::Refund {
            maker: c.maker.pubkey(),
            mint_a: c.mint_a,
            maker_ata_a: c.maker_ata_a,
            escrow: c.escrow,
            vault: c.vault,
            associated_token_program: anchor_spl::associated_token::ID,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: escrow::instruction::Refund {}.data(),
    }
}

fn update_ix(c: &Ctx, signer: &Pubkey, new_receive: u64) -> Instruction {
    Instruction {
        program_id: escrow::ID,
        accounts: escrow::accounts::Update {
            maker: *signer,
            escrow: c.escrow,
        }
        .to_account_metas(None),
        data: escrow::instruction::Update { new_receive }.data(),
    }
}

#[test]
fn initialize_creates_escrow_and_empty_vault() {
    let mut c = setup();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &c.maker.insecure_clone()).unwrap();

    let state = read_escrow(&c.svm, &c.escrow);
    assert_eq!(state.seed, SEED);
    assert_eq!(state.maker, c.maker.pubkey());
    assert_eq!(state.mint_a, c.mint_a);
    assert_eq!(state.mint_b, c.mint_b);
    assert_eq!(state.receive, RECEIVE);
    assert_eq!(balance(&c.svm, &c.vault), 0);
    assert_eq!(balance(&c.svm, &c.maker_ata_a), MAKER_A);
}

#[test]
fn initialize_rejects_zero_receive() {
    let mut c = setup();
    let ix = initialize_ix(&c, 0);
    assert!(send(&mut c.svm, ix, &c.maker.insecure_clone()).is_err());
    assert!(is_closed(&c.svm, &c.escrow));
}

#[test]
fn make_moves_tokens_into_vault() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = make_ix(&c, DEPOSIT);
    send(&mut c.svm, ix, &maker).unwrap();

    assert_eq!(balance(&c.svm, &c.vault), DEPOSIT);
    assert_eq!(balance(&c.svm, &c.maker_ata_a), MAKER_A - DEPOSIT);
}

#[test]
fn make_rejects_zero_deposit() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = make_ix(&c, 0);
    assert!(send(&mut c.svm, ix, &maker).is_err());
}

#[test]
fn take_swaps_tokens_and_closes_accounts() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let taker = c.taker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = make_ix(&c, DEPOSIT);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = take_ix(&c);
    send(&mut c.svm, ix, &taker).unwrap();

    let taker_ata_a = ata(&taker.pubkey(), &c.mint_a);
    let maker_ata_b = ata(&maker.pubkey(), &c.mint_b);
    assert_eq!(balance(&c.svm, &taker_ata_a), DEPOSIT);
    assert_eq!(balance(&c.svm, &maker_ata_b), RECEIVE);
    assert_eq!(balance(&c.svm, &c.taker_ata_b), TAKER_B - RECEIVE);
    assert!(is_closed(&c.svm, &c.escrow));
    assert!(is_closed(&c.svm, &c.vault));
}

#[test]
fn take_fails_on_empty_vault() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let taker = c.taker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = take_ix(&c);
    assert!(send(&mut c.svm, ix, &taker).is_err());
    assert_eq!(balance(&c.svm, &c.taker_ata_b), TAKER_B);
}

#[test]
fn take_fails_when_taker_cannot_pay() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let taker = c.taker.insecure_clone();
    let ix = initialize_ix(&c, TAKER_B + 1);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = make_ix(&c, DEPOSIT);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = take_ix(&c);
    assert!(send(&mut c.svm, ix, &taker).is_err());
    assert_eq!(balance(&c.svm, &c.vault), DEPOSIT);
}

#[test]
fn refund_returns_tokens_and_closes_accounts() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = make_ix(&c, DEPOSIT);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = refund_ix(&c);
    send(&mut c.svm, ix, &maker).unwrap();

    assert_eq!(balance(&c.svm, &c.maker_ata_a), MAKER_A);
    assert!(is_closed(&c.svm, &c.escrow));
    assert!(is_closed(&c.svm, &c.vault));
}

#[test]
fn refund_works_on_empty_vault() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = refund_ix(&c);
    send(&mut c.svm, ix, &maker).unwrap();

    assert_eq!(balance(&c.svm, &c.maker_ata_a), MAKER_A);
    assert!(is_closed(&c.svm, &c.escrow));
    assert!(is_closed(&c.svm, &c.vault));
}

#[test]
fn refund_by_non_maker_fails() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = make_ix(&c, DEPOSIT);
    send(&mut c.svm, ix, &maker).unwrap();

    let attacker = c.taker.insecure_clone();
    let mut ix = refund_ix(&c);
    ix.accounts[0].pubkey = attacker.pubkey();
    assert!(send(&mut c.svm, ix, &attacker).is_err());
    assert_eq!(balance(&c.svm, &c.vault), DEPOSIT);
}

#[test]
fn update_changes_receive() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = update_ix(&c, &maker.pubkey(), 250);
    send(&mut c.svm, ix, &maker).unwrap();

    assert_eq!(read_escrow(&c.svm, &c.escrow).receive, 250);
}

#[test]
fn update_rejects_zero() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = update_ix(&c, &maker.pubkey(), 0);
    assert!(send(&mut c.svm, ix, &maker).is_err());
    assert_eq!(read_escrow(&c.svm, &c.escrow).receive, RECEIVE);
}

#[test]
fn update_by_non_maker_fails() {
    let mut c = setup();
    let maker = c.maker.insecure_clone();
    let attacker = c.taker.insecure_clone();
    let ix = initialize_ix(&c, RECEIVE);
    send(&mut c.svm, ix, &maker).unwrap();
    let ix = update_ix(&c, &attacker.pubkey(), 1);
    assert!(send(&mut c.svm, ix, &attacker).is_err());
    assert_eq!(read_escrow(&c.svm, &c.escrow).receive, RECEIVE);
}
