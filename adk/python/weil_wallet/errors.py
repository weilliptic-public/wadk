"""Errors for the WeilChain wallet SDK."""


class InvalidContractIdError(Exception):
    """Raised when a contract ID string is invalid."""

    def __init__(self, msg: str) -> None:
        self.msg = msg
        super().__init__(f"invalid contract id: {msg}")


class WalletNotPermittedError(Exception):
    """Raised when a wallet address is not authorised to call a secured tool.

    Attributes:
        wallet_addr: The 0x-prefixed address that was rejected.
        svc_name: The applet service name the tool was guarded by.
    """

    def __init__(self, wallet_addr: str, svc_name: str) -> None:
        self.wallet_addr = wallet_addr
        self.svc_name = svc_name
        super().__init__(
            f"wallet {wallet_addr!r} does not have permission to call tools "
            f"secured by applet {svc_name!r}"
        )
