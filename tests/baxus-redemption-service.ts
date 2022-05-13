import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { TOKEN_PROGRAM_ID, Token } from '@solana/spl-token';
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram, Connection } from '@solana/web3.js';
import { BaxusRedemptionService } from '../target/types/baxus_redemption_service';
import * as assert from 'assert'
import { NodeWallet } from '@project-serum/anchor/dist/cjs/provider';
import { findProgramAddressSync } from '@project-serum/anchor/dist/cjs/utils/pubkey';

describe('baxus-redemption-service', () => {

  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.Provider.env());

  const program = anchor.workspace.BaxusRedemptionService as Program<BaxusRedemptionService>;

  let testCustomerTokenAccount: PublicKey = null;

  let testTokenMintAccount: Token = null;

  let testRedemptionInfoAccount: PublicKey = null;
  let testRedemptionBump: number = null;

  let testBaxusEscrowAccount: PublicKey = null;
  let testEscrowBump: number = null;

  it('Basic test for initialize_redemption():', async () => { 

    // Create a Token Mint Account 
    testTokenMintAccount = await Token.createMint(
      program.provider.connection, 
      (program.provider.wallet as NodeWallet).payer,
      program.provider.wallet.publicKey,
      null,
      0,
      TOKEN_PROGRAM_ID);

    // Create an Associated Token Account using that Mint and the program.provider.wallet (i.e. this test's wallet) as the owner
    testCustomerTokenAccount = await testTokenMintAccount.createAssociatedTokenAccount(
      program.provider.wallet.publicKey
    );

    // Mint a token to that ATA
    await testTokenMintAccount.mintTo(
      testCustomerTokenAccount,
      program.provider.wallet.publicKey,
      [],
      1)

    // Check that the testCustomerTokenAccount has one token in it
    assert.equal(1, (await testTokenMintAccount.getAccountInfo(testCustomerTokenAccount)).amount.toNumber());

    // Create an address at which the RedemptionInfo account used by this test will live
    [testRedemptionInfoAccount, testRedemptionBump] = await anchor.web3.PublicKey.findProgramAddress(
      [testTokenMintAccount.publicKey.toBuffer(), Buffer.from("redemption")],
      program.programId,
    );

    // Create a PDA at which the BAXUS Escrow Account used by this test will live - also generate the Escrow Bump
    [testBaxusEscrowAccount, testEscrowBump] = await anchor.web3.PublicKey.findProgramAddress(
      [testTokenMintAccount.publicKey.toBuffer()],
      program.programId,
    );

    const tx = await program.rpc.initializeRedemption({
      accounts: {
        redemptionInfo: testRedemptionInfoAccount,
        customerTokenAccount: testCustomerTokenAccount,
        customerPaymentAccount: program.provider.wallet.publicKey,
        tokenMintAccount: testTokenMintAccount.publicKey,
        baxusEscrowAccount: testBaxusEscrowAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        systemProgram: SystemProgram.programId,
      },
      signers: []
    });

    // Confirm that the testTokenMintAccount has 1 token in circulation
    assert.equal(1, ( await testTokenMintAccount.getMintInfo() ).supply);

    // Check that the testCustomerTokenAccount is empty
    assert.equal(0, (await testTokenMintAccount.getAccountInfo(testCustomerTokenAccount)).amount.toNumber());

    // Check that the testBaxusEscrowAccount has one token in it
    assert.equal(1, (await testTokenMintAccount.getAccountInfo(testBaxusEscrowAccount)).amount.toNumber());

    console.log("Your transaction signature", tx);
  });

  it('Basic test for return_asset_token():', async () => {

    const tx = await program.rpc.returnAssetToken({
      accounts: {
        redemptionInfo: testRedemptionInfoAccount,
        customerTokenAccount: testCustomerTokenAccount,
        customerPaymentAccount: program.provider.wallet.publicKey,
        tokenMintAccount: testTokenMintAccount.publicKey,
        baxusEscrowAccount: testBaxusEscrowAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
      signers: []
    });

    // Confirm that the testTokenMintAccount still has 1 token in circulation
    assert.equal(1, ( await testTokenMintAccount.getMintInfo() ).supply);

    // Check that the testCustomerTokenAccount has one token in it
    assert.equal(1, (await testTokenMintAccount.getAccountInfo(testCustomerTokenAccount)).amount.toNumber());

    // Check that the testRedemptionInfoAccount and testBaxusEscrowAccount were closed 
    assert.equal(null, await program.provider.connection.getAccountInfo(testRedemptionInfoAccount));
    assert.equal(null, await program.provider.connection.getAccountInfo(testBaxusEscrowAccount));

    console.log("Your transaction signature", tx);
  });

  it('Basic test for burn_asset_token():', async () => {

    // Create a new address at which the RedemptionInfo account used by this test will live (since we closed the RedemptionInfo account in the last test)
    [testRedemptionInfoAccount, testRedemptionBump] = await anchor.web3.PublicKey.findProgramAddress(
      [testTokenMintAccount.publicKey.toBuffer(), Buffer.from("redemption")],
      program.programId,
    );

    // Create a new PDA at which the BAXUS Escrow Account used by this test will live - also generate a new Escrow Bump
    [testBaxusEscrowAccount, testEscrowBump] = await anchor.web3.PublicKey.findProgramAddress(
      [testTokenMintAccount.publicKey.toBuffer()],
      program.programId,
    );

    const tx1 = await program.rpc.initializeRedemption({
      accounts: {
        redemptionInfo: testRedemptionInfoAccount,
        customerTokenAccount: testCustomerTokenAccount,
        customerPaymentAccount: program.provider.wallet.publicKey,
        tokenMintAccount: testTokenMintAccount.publicKey,
        baxusEscrowAccount: testBaxusEscrowAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        systemProgram: SystemProgram.programId,
      },
      // The testRedemptionInfoAccount has to sign upon its own creation
      signers: []
    });

    // Perform the same checks as in the first test
    assert.equal(1, ( await testTokenMintAccount.getMintInfo() ).supply);
    assert.equal(0, (await testTokenMintAccount.getAccountInfo(testCustomerTokenAccount)).amount.toNumber());
    assert.equal(1, (await testTokenMintAccount.getAccountInfo(testBaxusEscrowAccount)).amount.toNumber());
   
    const tx2 = await program.rpc.burnAssetToken({
      accounts: {
        redemptionInfo: testRedemptionInfoAccount,
        customerTokenAccount: testCustomerTokenAccount,
        customerPaymentAccount: program.provider.wallet.publicKey,
        tokenMintAccount: testTokenMintAccount.publicKey,
        baxusEscrowAccount: testBaxusEscrowAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      },
      signers: []
    });

    // Confirm that the testTokenMintAccount has no more tokens left in circulation
    assert.equal(0, ( await testTokenMintAccount.getMintInfo() ).supply);

    // Check that the testRedemptionInfoAccount and testBaxusEscrowAccount were closed 
    assert.equal(null, await program.provider.connection.getAccountInfo(testRedemptionInfoAccount));
    assert.equal(null, await program.provider.connection.getAccountInfo(testBaxusEscrowAccount));

    console.log("Your transaction signature", tx2);
  });

});
