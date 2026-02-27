package contract

import (
	"bytes"
	"encoding/base32"
	"encoding/binary"
	"fmt"
	"strings"
)

const (
	contractIDDecodedLen = 36 // decoded base32 contract ID is always 36 bytes
	podCounterLen        = 4  // pod counter occupies the first 4 bytes
)

// input byteArray should be of length 4
func byteArrayToInt(byteArray []byte) (int32, error) {
	var num int32
	buf := bytes.NewReader(byteArray)
	err := binary.Read(buf, binary.BigEndian, &num)

	if err != nil {
		return 0, err
	}

	return num, nil
}

// Gets the Pod Counter from the id <string>
func PodCounter(contractId string) (int, error) {
	decoded, err := base32.StdEncoding.WithPadding(base32.NoPadding).DecodeString(strings.ToUpper(contractId))

	if err != nil {
		return 0, err
	}

	if len(decoded) != contractIDDecodedLen {
		return 0, fmt.Errorf("invalid contract-id: expected %d bytes, got %d", contractIDDecodedLen, len(decoded))
	}

	podIdBytes := decoded[:podCounterLen]
	podIdCounter, err := byteArrayToInt(podIdBytes)

	if err != nil {
		return 0, err
	}

	return int(podIdCounter), nil
}
