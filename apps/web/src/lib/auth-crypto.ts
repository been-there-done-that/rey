import {
  generate_key_b64,
  generate_keypair_b64,
  generate_salt_b64,
  derive_kek_b64,
  derive_verification_key_b64,
  bcrypt_hash_b64,
  encrypt_key_b64,
} from "@/wasm-pkg/index"

export async function generateKey() {
  return generate_key_b64()
}

export async function generateKeypair() {
  return JSON.parse(generate_keypair_b64()) as {
    public_key: string
    secret_key: string
  }
}

export async function generateSalt() {
  return generate_salt_b64()
}

export async function deriveKek(password: string, salt: string) {
  return derive_kek_b64(password, salt, 67108864, 2)
}

export async function deriveVerificationKey(kek: string) {
  return derive_verification_key_b64(kek)
}

export async function bcryptHash(plaintextB64: string) {
  return bcrypt_hash_b64(plaintextB64)
}

export async function encryptKey(plaintextB64: string, wrappingB64: string) {
  return JSON.parse(encrypt_key_b64(plaintextB64, wrappingB64)) as {
    nonce: string
    ciphertext: string
  }
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
