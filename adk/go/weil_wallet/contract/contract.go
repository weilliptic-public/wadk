package contract

import (
	"bytes"
	"encoding/binary"
	"encoding/hex"
	"fmt"
)

//  input byteArray should be of length 4
func byteArrayToInt(byteArray []byte) (int32, error) {
	var num int32
	buf := bytes.NewReader(byteArray)
	err := binary.Read(buf, binary.BigEndian, &num)

	if err != nil {
		return 0, err
	}

	return num, nil
}

//  Gets the Pod Counter from the id <string>
func PodCounter(contractId string) (int, error) {
	decodedBytes, err := hex.DecodeString(contractId)
	if err != nil {
		return 0, err
	}

	if len(decodedBytes) != 36 {
		return 0, fmt.Errorf("invalid contract-id: Expected 36 bytes long , got %v bytes", len(decodedBytes))
	}

	podIdBytes := decodedBytes[:4]
	podIdCounter, err := byteArrayToInt(podIdBytes)

	if err != nil {
		return 0, err
	}

	return int(podIdCounter), nil
}
