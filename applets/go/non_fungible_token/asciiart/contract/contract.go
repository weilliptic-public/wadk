package contract

import (
	"fmt"
	"strconv"

	"github.com/weilliptic-public/wadk/adk/go/weil_go/collections"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/runtime"
	"github.com/weilliptic-public/wadk/adk/go/weil_go/weil_contracts/non_fungible"
)

type AsciiArtContractState struct {
	/// Controllers allowed to mint new tokens.
	/// TODO: How is the set updated?
	Controllers collections.WeilSet[string]
	Inner       non_fungible.NonFungibleToken
}

func NewAsciiArtContractState() (*AsciiArtContractState, error) {
	creator := runtime.Sender()
	controllers := *collections.NewWeilSet[string](*collections.NewWeilId(0))
	controllers.Insert(&creator)

	token := AsciiArtContractState{
		Controllers: controllers,
		Inner:       *non_fungible.NewNonFungibleToken("AsciiArt"),
	}

	initialTokens := [6]non_fungible.Token{
		{
			Title:       "A fish going left!",
			Name:        "fish 1",
			Description: "A one line ASCII drawing of a fish",
			Payload:     "<><",
		},
		{
			Title:       "A fish going right!",
			Name:        "fish 2",
			Description: "A one line ASCII drawing of a fish swimming to the right",
			Payload:     "><>",
		},
		{
			Title:       "A big fish going left!",
			Name:        "fish 3",
			Description: "A one line ASCII drawing of a fish swimming to the left",
			Payload:     "<'))><",
		},
		{
			Title:       "A big fish going right!",
			Name:        "fish 4",
			Description: "A one line ASCII drawing of a fish swimming to the right",
			Payload:     "><(('>",
		},
		{
			Title:       "A Face",
			Name:        "face 1",
			Description: "A one line ASCII drawing of a face",
			Payload:     "(-_-)",
		},
		{
			Title:       "Arms raised",
			Name:        "arms 1",
			Description: "A one line ASCII drawing of a person with arms raised",
			Payload:     "\\o/",
		},
	}

	for i, t := range initialTokens {
		err := token.Inner.Mint(strconv.Itoa(i), t)
		if err != nil {
			return nil, err
		}
	}

	return &token, nil
}

// query
func (obj *AsciiArtContractState) Name() string {
	return obj.Inner.Name()
}

// query
func (obj *AsciiArtContractState) BalanceOf(addr string) uint64 {
	return obj.Inner.BalanceOf(addr)
}

// query
func (obj *AsciiArtContractState) OwnerOf(tokenId string) (*string, error) {
	return obj.Inner.OwnerOf(tokenId)
}

// query
func (obj *AsciiArtContractState) Details(tokenId string) (*TokenDetails, error) {
	result, err := obj.Inner.Details(tokenId)
	if err != nil {
		return nil, err
	}

	return &TokenDetails{
		Name:        result.Name,
		Title:       result.Title,
		Description: result.Description,
		Payload:     result.Payload,
	}, nil
}

// mutate
func (obj *AsciiArtContractState) Approve(spender string, tokenId string) error {
	return obj.Inner.Approve(spender, tokenId)
}

// mutate
func (obj *AsciiArtContractState) SetApproveForAll(spender string, approval bool) {
	obj.Inner.SetApproveForAll(spender, approval)
}

// mutate
func (obj *AsciiArtContractState) Transfer(toAddr string, tokenId string) error {
	return obj.Inner.Transfer(toAddr, tokenId)
}

// mutate
func (obj *AsciiArtContractState) TransferFrom(fromAddr string, toAddr string, tokenId string) error {
	return obj.Inner.TransferFrom(fromAddr, toAddr, tokenId)
}

// query
func (obj *AsciiArtContractState) GetApproved(tokenId string) (*[]string, error) {
	return obj.Inner.GetApproved(tokenId)
}

// query
func (obj *AsciiArtContractState) IsApprovedForAll(owner string, spender string) bool {
	return obj.Inner.IsApprovedForAll(owner, spender)
}

func (t *AsciiArtContractState) isController(addr string) bool {
	return t.Controllers.Contains(&addr)
}

// mutate
func (obj *AsciiArtContractState) Mint(tokenId string, title string, name string, description string, payload string) error {
	fromAddr := runtime.Sender()

	token := non_fungible.Token{
		Title:       title,
		Name:        name,
		Description: description,
		Payload:     payload,
	}

	if !obj.isController(fromAddr) {
		return fmt.Errorf("only controllers can mint")
	}

	return obj.Inner.Mint(tokenId, token)
}
