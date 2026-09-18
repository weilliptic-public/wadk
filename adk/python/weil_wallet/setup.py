import sys

from setuptools import Extension, setup


extensions = []
if sys.platform.startswith("linux"):
    extensions.append(
        Extension(
            "weil_wallet._secure_wallet",
            sources=[
                "secure_native/python_binding.c",
                "secure_native/weil_secure_wallet.c",
            ],
            libraries=["crypto", "curl"],
            extra_compile_args=["-std=c11"],
        )
    )

setup(ext_modules=extensions)
