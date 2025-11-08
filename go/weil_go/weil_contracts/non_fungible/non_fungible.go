package non_fungible

import (
	"fmt"

	"unicode/utf8"

	"github.com/lasarocamargos/jsonmap"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/collections"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/ledger"
	"github.com/weilliptic-inc/contract-sdk/go/weil_go/runtime"
)

type Token struct {
	Title       string `json:"title"`
	Name        string `json:"name"`
	Description string `json:"description"`
	Payload     string `json:"payload"`
}

type innerNonFungibleToken struct {
	Name       string                                   `json:"name"`
	Creator    string                                   `json:"creator"`
	Id2Tokens  collections.WeilMap[string, Token]       `json:"tokens"`
	Id2Owner   collections.WeilMap[string, string]      `json:"owners"`
	Owner2Ids  collections.WeilMap[string, jsonmap.Map] `json:"owned"`
	Allowances collections.WeilMap[string, string]      `json:"allowances"`
}

type NonFungibleToken struct {
	Inner innerNonFungibleToken `json:"token"`
}

func (i *innerNonFungibleToken) Dump() string {
	return fmt.Sprintf("Tokens: %s, Owners: %s, Owned: %s, Allowances: %s",
		i.Id2Tokens.BaseStatePath(),
		i.Id2Owner.BaseStatePath(),
		i.Owner2Ids.BaseStatePath(),
		i.Allowances.BaseStatePath(),
	)
}

func NewNonFungibleToken(name string) *NonFungibleToken {
	return &NonFungibleToken{
		Inner: innerNonFungibleToken{
			Name:       name,
			Creator:    runtime.Sender(),
			Id2Tokens:  *collections.NewWeilMap[string, Token](*collections.NewWeilId(0)),
			Id2Owner:   *collections.NewWeilMap[string, string](*collections.NewWeilId(10)),
			Owner2Ids:  *collections.NewWeilMap[string, jsonmap.Map](*collections.NewWeilId(2)),
			Allowances: *collections.NewWeilMap[string, string](*collections.NewWeilId(3)),
		},
	}
}

func (t *NonFungibleToken) Name() string {
	return t.Inner.Name
}

func (t *NonFungibleToken) Creator() string {
	return string(t.Inner.Creator)
}

func isValidId(tokenId string) bool {
	len := utf8.RuneCountInString(tokenId)
	return len > 0 && len < 255
}

func (t *NonFungibleToken) hasBeenMinted(tokenId string) bool {
	val, err := t.Inner.Id2Owner.Get(&tokenId)
	if err != nil {
		return false
	}
	return *val != ""
}

func (t *NonFungibleToken) BalanceOf(addr string) uint64 {
	val, err := t.Inner.Owner2Ids.Get(&addr)
	if err != nil {
		return 0
	}
	return uint64(val.Len())
}

func (t *NonFungibleToken) OwnerOf(tokenId string) (*string, error) {
	if !isValidId(tokenId) {
		return nil, fmt.Errorf("`%s` is not a valid token id", tokenId)
	}

	val, err := t.Inner.Id2Owner.Get(&tokenId)
	if err != nil {
		return nil, fmt.Errorf("owner of `%s` is not identified", tokenId)
	}
	return val, nil
}

func (t *NonFungibleToken) Details(tokenId string) (*Token, error) {
	if !isValidId(tokenId) {
		return nil, fmt.Errorf("`%s` is not a valid token id", tokenId)
	}

	if !t.hasBeenMinted(tokenId) {
		return nil, fmt.Errorf("token `%s` has not been minted", tokenId)
	}

	val, err := t.Inner.Id2Tokens.Get(&tokenId)
	if err != nil {
		return nil, fmt.Errorf("token `%s` not found", tokenId)
	}
	return val, nil
}

func (t *NonFungibleToken) doTransfer(tokenId string, fromAddr string, toAddr string) error {
	// Update the Ledger
	err := ledger.Transfer(tokenId, fromAddr, toAddr, 1)
	if err != nil {
		return fmt.Errorf("`%s` could not be transferred by the Ledger", tokenId)
	}

	// Update the token
	t.Inner.Id2Owner.Insert(&tokenId, &toAddr)

	// Update the owners
	oldOwnersTokens, err := t.Inner.Owner2Ids.Get(&fromAddr)
	if err != nil {
		return fmt.Errorf("owned token is missing")
	}
	oldOwnersTokens.Delete(tokenId)
	t.Inner.Owner2Ids.Insert(&fromAddr, oldOwnersTokens)

	newOwnersTokens, err := t.Inner.Owner2Ids.Get(&toAddr)
	if err != nil {
		newOwnersTokens = jsonmap.New()
	}
	newOwnersTokens.Set(tokenId, nil)
	t.Inner.Owner2Ids.Insert(&toAddr, newOwnersTokens)

	// Update old owner's individual allowances
	key := fmt.Sprintf("%s$%s", fromAddr, tokenId)
	_, _ = t.Inner.Allowances.Remove(&key)
	return nil
}

func (t *NonFungibleToken) Transfer(toAddr string, tokenId string) error {
	fromAddr := runtime.Sender()

	if !isValidId(tokenId) {
		return fmt.Errorf("`%s` is not a valid token id", tokenId)
	}

	oldOwner, err := t.Inner.Id2Owner.Get(&tokenId)
	if err != nil {
		return fmt.Errorf("owner of `%s` is not identified", tokenId)
	}

	if *oldOwner != fromAddr {
		return fmt.Errorf("token `%s` now owned by `%s`", tokenId, fromAddr)

	}
	return t.doTransfer(tokenId, fromAddr, toAddr)
}

func (t *NonFungibleToken) TransferFrom(fromAddr string, toAddr string, tokenId string) error {
	spender := runtime.Sender()

	if !isValidId(tokenId) {
		return fmt.Errorf("`%s` is not a valid token id", tokenId)
	}

	oldOwner, err := t.Inner.Id2Owner.Get(&tokenId)
	if err != nil {
		return fmt.Errorf("owner of `%s` is not identified", tokenId)
	}

	if *oldOwner != fromAddr {
		return fmt.Errorf("token `%s` not owned by `%s`", tokenId, fromAddr)
	}

	key := fmt.Sprintf("%s$%s", *oldOwner, tokenId)

	var allowed bool
	allowance, err := t.Inner.Allowances.Get(&key)
	if err != nil {
		allowed = false
	} else {
		allowed = *allowance == spender
	}

	if !allowed {
		key = fmt.Sprintf("%s$", *oldOwner)

		allowance, err = t.Inner.Allowances.Get(&key)
		if err != nil {
			allowed = false
		} else {
			allowed = *allowance == spender
		}
	}

	if !allowed {
		return fmt.Errorf("transfer of token `%s` not authorized", tokenId)
	}

	return t.doTransfer(tokenId, fromAddr, toAddr)
}

func (t *NonFungibleToken) Approve(spender string, tokenId string) error {
	fromAddr := runtime.Sender()

	if !isValidId(tokenId) {
		return fmt.Errorf("`%s` is not a valid token id", tokenId)
	}

	oldOwner, err := t.Inner.Id2Owner.Get(&tokenId)
	if err != nil {
		return fmt.Errorf("owner of `%s` is not identified", tokenId)
	}

	if *oldOwner != fromAddr {
		return fmt.Errorf("allowance of token `%s` not authorized", tokenId)
	}

	key := fmt.Sprintf("%s$%s", fromAddr, tokenId)
	if spender == "" {
		t.Inner.Allowances.Remove(&key)
	} else {
		t.Inner.Allowances.Insert(&key, &spender)
	}
	return nil
}

func (t *NonFungibleToken) SetApproveForAll(spender string, approval bool) {
	fromAddr := runtime.Sender()

	key := fmt.Sprintf("%s$", fromAddr)
	if approval {
		t.Inner.Allowances.Insert(&key, &spender)
	} else {
		t.Inner.Allowances.Remove(&key)
	}
}

func (t *NonFungibleToken) GetApproved(tokenId string) (*[]string, error) {
	response := make([]string, 0)
	if !isValidId(tokenId) {
		return nil, fmt.Errorf("`%s` is not a valid token id", tokenId)
	}

	owner, err := t.Inner.Id2Owner.Get(&tokenId)
	if err != nil {
		return nil, fmt.Errorf("owner of `%s` is not identified", tokenId)
	}

	key := fmt.Sprintf("%s$%s", *owner, tokenId)
	allowance, err := t.Inner.Allowances.Get(&key)
	if err == nil {
		response = append(response, *allowance)
	}
	key = fmt.Sprintf("%s$", *owner)
	allowance, err = t.Inner.Allowances.Get(&key)
	if err == nil {
		response = append(response, *allowance)
	}
	return &response, nil
}

func (t *NonFungibleToken) IsApprovedForAll(owner string, spender string) bool {
	key := fmt.Sprintf("%s$", owner)

	allowance, err := t.Inner.Allowances.Get(&key)
	if err != nil {
		return false
	}
	return *allowance == spender
}

func (t *NonFungibleToken) Mint(tokenId string, token Token) error {
	fromAddr := runtime.Sender()

	if !isValidId(tokenId) {
		return fmt.Errorf("`%s` is not a valid token id", tokenId)
	}

	if t.hasBeenMinted(tokenId) {
		return fmt.Errorf("token `%s` has already been minted", tokenId)
	}

	err := ledger.Mint(tokenId, fromAddr, 1)
	if err != nil {
		return fmt.Errorf("`%s` could not be transferred by the Ledger", tokenId)
	}

	t.Inner.Id2Tokens.Insert(&tokenId, &token)
	t.Inner.Id2Owner.Insert(&tokenId, &fromAddr)

	newOwnersTokens, err := t.Inner.Owner2Ids.Get(&fromAddr)
	if err != nil {
		newOwnersTokens = jsonmap.New()
	}

	newOwnersTokens.Set(tokenId, nil)
	t.Inner.Owner2Ids.Insert(&fromAddr, newOwnersTokens)
	return nil
}
