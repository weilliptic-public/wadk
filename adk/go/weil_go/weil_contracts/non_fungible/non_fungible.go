// Package non_fungible provides a non-fungible token (NFT) implementation for Weil contracts.
// It implements an ERC-721-like token standard with unique token IDs, ownership tracking,
// and approval mechanisms.
package non_fungible

import (
	"fmt"

	"unicode/utf8"

	"github.com/weilliptic-public/jsonmap"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/collections"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/ledger"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
)

// Token represents the metadata for a non-fungible token.
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

// NonFungibleToken represents a non-fungible token contract.
type NonFungibleToken struct {
	Inner innerNonFungibleToken `json:"token"`
}

// Dump returns a string representation of the internal state paths for debugging.
func (i *innerNonFungibleToken) Dump() string {
	return fmt.Sprintf("Tokens: %s, Owners: %s, Owned: %s, Allowances: %s",
		i.Id2Tokens.BaseStatePath(),
		i.Id2Owner.BaseStatePath(),
		i.Owner2Ids.BaseStatePath(),
		i.Allowances.BaseStatePath(),
	)
}

// NewNonFungibleToken creates a new non-fungible token contract with the given name.
// The creator is set to the address of the transaction sender.
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

// Name returns the name of the NFT contract.
func (t *NonFungibleToken) Name() string {
	return t.Inner.Name
}

// Creator returns the address of the contract creator.
func (t *NonFungibleToken) Creator() string {
	return string(t.Inner.Creator)
}

// isValidId checks if a token ID is valid (non-empty and less than 255 characters).
func isValidId(tokenId string) bool {
	len := utf8.RuneCountInString(tokenId)
	return len > 0 && len < 255
}

// hasBeenMinted checks if a token with the given ID has already been minted.
func (t *NonFungibleToken) hasBeenMinted(tokenId string) bool {
	val, err := t.Inner.Id2Owner.Get(&tokenId)
	if err != nil {
		return false
	}
	return *val != ""
}

// BalanceOf returns the number of tokens owned by the given address.
func (t *NonFungibleToken) BalanceOf(addr string) uint64 {
	val, err := t.Inner.Owner2Ids.Get(&addr)
	if err != nil {
		return 0
	}
	return uint64(val.Len())
}

// OwnerOf returns the address of the owner of the token with the given ID.
// Returns an error if the token ID is invalid or the token has not been minted.
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

// Details returns the metadata for the token with the given ID.
// Returns an error if the token ID is invalid or the token has not been minted.
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

// doTransfer performs the internal transfer logic for moving a token from one address to another.
// This is a helper function used by Transfer and TransferFrom.
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

// Transfer transfers a token from the sender's address to the specified address.
// Returns an error if the token ID is invalid, the token is not owned by the sender, or the transfer fails.
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

// TransferFrom transfers a token from one address to another on behalf of the sender.
// The sender must be the owner or have been approved to transfer the token.
// Returns an error if the transfer is not authorized or fails.
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

// Approve approves the spender to transfer the specified token.
// Only the owner of the token can approve a spender.
// If spender is an empty string, the approval is removed.
// Returns an error if the token ID is invalid or the sender is not the owner.
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

// SetApproveForAll approves or revokes approval for the spender to transfer all tokens
// owned by the sender. If approval is true, the spender is approved; if false, approval is revoked.
func (t *NonFungibleToken) SetApproveForAll(spender string, approval bool) {
	fromAddr := runtime.Sender()

	key := fmt.Sprintf("%s$", fromAddr)
	if approval {
		t.Inner.Allowances.Insert(&key, &spender)
	} else {
		t.Inner.Allowances.Remove(&key)
	}
}

// GetApproved returns a list of approved spenders for the given token.
// This includes both token-specific approvals and operator approvals (approve for all).
// Returns an error if the token ID is invalid or the token has not been minted.
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

// IsApprovedForAll checks if the spender is approved to transfer all tokens owned by the owner.
func (t *NonFungibleToken) IsApprovedForAll(owner string, spender string) bool {
	key := fmt.Sprintf("%s$", owner)

	allowance, err := t.Inner.Allowances.Get(&key)
	if err != nil {
		return false
	}
	return *allowance == spender
}

// Mint creates a new token with the given ID and metadata, assigning it to the sender.
// Returns an error if the token ID is invalid, the token has already been minted, or minting fails.
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
