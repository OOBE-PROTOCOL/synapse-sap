const anchor = require('@coral-xyz/anchor');
const { Connection, Keypair, PublicKey } = require('@solana/web3.js');
const fs = require('fs');

(async () => {
  // Load keypair (upgrade authority)
  const raw = JSON.parse(fs.readFileSync('/Users/keepeeto/.config/solana/id.json', 'utf8'));
  const wallet = new anchor.Wallet(Keypair.fromSecretKey(Uint8Array.from(raw)));

  // Connect to mainnet
  const connection = new Connection('https://api.mainnet-beta.solana.com', 'confirmed');
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: 'confirmed' });
  anchor.setProvider(provider);

  // Load IDL and program (override address to new program ID)
  const idl = require('../target/idl/synapse_agent_sap.json');
  const programId = new PublicKey('SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ');
  idl.address = programId.toBase58();
  const program = new anchor.Program(idl, provider);

  console.log('Authority:', wallet.publicKey.toBase58());
  console.log('Program ID:', programId.toBase58());

  // Derive PDA
  const [globalRegistry] = PublicKey.findProgramAddressSync(
    [Buffer.from('sap_global')],
    programId
  );
  console.log('Global Registry PDA:', globalRegistry.toBase58());

  // Check balance
  const balance = await connection.getBalance(wallet.publicKey);
  console.log('Balance:', balance / 1e9, 'SOL');

  // Call initialize_global
  const tx = await program.methods
    .initializeGlobal()
    .accounts({
      authority: wallet.publicKey,
      globalRegistry: globalRegistry,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

  console.log('Transaction:', tx);
  console.log('Global registry initialized successfully!');
})().catch(err => {
  console.error('Error:', err.message || err);
  process.exit(1);
});
