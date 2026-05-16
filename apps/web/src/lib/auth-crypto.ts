import { initWasm, getWasm } from "./wasm-loader"

let initPromise: Promise<void> | null = null

async function ensureWasm() {
  if (!initPromise) {
    initPromise = initWasm()
  }
  return initPromise
}

export async function generateKey() {
  await ensureWasm()
  return getWasm().generate_key_b64()
}

export async function generateKeypair() {
  await ensureWasm()
  return JSON.parse(await getWasm().generate_keypair_b64()) as {
    public_key: string
    secret_key: string
  }
}

export async function generateSalt() {
  await ensureWasm()
  return getWasm().generate_salt_b64()
}

export async function deriveKek(password: string, salt: string) {
  await ensureWasm()
  return getWasm().derive_kek_b64(password, salt, 67108864, 2)
}

export async function deriveVerificationKey(kek: string) {
  await ensureWasm()
  return getWasm().derive_verification_key_b64(kek)
}

export async function bcryptHash(plaintextB64: string) {
  await ensureWasm()
  return getWasm().bcrypt_hash_b64(plaintextB64)
}

export async function encryptKey(plaintextB64: string, wrappingB64: string) {
  await ensureWasm()
  return JSON.parse(await getWasm().encrypt_key_b64(plaintextB64, wrappingB64)) as {
    nonce: string
    ciphertext: string
  }
}

export async function streamEncrypt(dataB64: string, keyB64: string) {
  await ensureWasm()
  return JSON.parse(await getWasm().stream_encrypt_b64(dataB64, keyB64)) as {
    header: string
    ciphertext: string
  }
}

export async function streamDecrypt(headerB64: string, ciphertextB64: string, keyB64: string) {
  await ensureWasm()
  return getWasm().stream_decrypt_b64(headerB64, ciphertextB64, keyB64)
}

export async function prepareSignup(email: string, password: string) {
  const salt = await generateSalt()
  const kek = await deriveKek(password, salt)
  const verifyKey = await deriveVerificationKey(kek)
  const verifyKeyHash = await bcryptHash(verifyKey)

  const masterKey = await generateKey()
  const keypair = await generateKeypair()
  const encryptedMasterKey = await encryptKey(masterKey, kek)
  const encryptedSecretKey = await encryptKey(keypair.secret_key, kek)
  const encryptedRecoveryKey = await encryptKey(await generateKey(), kek)

  return {
    email,
    verify_key_hash: verifyKeyHash,
    encrypted_master_key: encryptedMasterKey.ciphertext,
    key_nonce: encryptedMasterKey.nonce,
    kek_salt: salt,
    mem_limit: 67108864,
    ops_limit: 2,
    public_key: keypair.public_key,
    encrypted_secret_key: encryptedSecretKey.ciphertext,
    secret_key_nonce: encryptedSecretKey.nonce,
    encrypted_recovery_key: encryptedRecoveryKey.ciphertext,
    recovery_key_nonce: encryptedRecoveryKey.nonce,
  }
}

export async function prepareLogin(email: string, password: string) {
  const verifyKey = await deriveVerificationKey(
    await deriveKek(password, "")
  )
  return { email, verify_key_hash: verifyKey }
}
