package utils

import (
	"crypto/sha256"
	"encoding/hex"

	"github.com/decred/dcrd/dcrec/secp256k1/v4"
)

func HashSha256(buffer []byte) []byte {
	h := sha256.New()
	h.Write(buffer)

	return h.Sum(nil)
}

func GetAddressFromPublicKey(publicKey secp256k1.PublicKey) string {
	addr := HashSha256(publicKey.SerializeUncompressed())

	return hex.EncodeToString(addr)
}
