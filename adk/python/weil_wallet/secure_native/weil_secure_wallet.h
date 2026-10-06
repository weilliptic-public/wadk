#ifndef WEIL_SECURE_WALLET_H
#define WEIL_SECURE_WALLET_H

#include <stddef.h>
#include <stdint.h>

#if defined(_WIN32)
#define WEIL_SECURE_API __declspec(dllexport)
#else
#define WEIL_SECURE_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define WEIL_SECURE_ERROR_SIZE 256
#define WEIL_SECURE_METADATA_SIZE 512

typedef struct weil_secure_wallet weil_secure_wallet;

typedef struct {
    int locked;
    int dontdump;
    int guard_pages;
    int wipeonfork;
} weil_secure_status;

typedef struct {
    char address[WEIL_SECURE_METADATA_SIZE];
    char organization[WEIL_SECURE_METADATA_SIZE];
    char subgroup[WEIL_SECURE_METADATA_SIZE];
    char purpose[WEIL_SECURE_METADATA_SIZE];
} weil_secure_metadata;

WEIL_SECURE_API int weil_secure_harden_process(
    char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API int weil_secure_wallet_from_key_fd(
    int fd, const char *address, int require_lock,
    weil_secure_wallet **out, char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API int weil_secure_wallet_from_wallet_fd(
    int fd, int require_lock, weil_secure_wallet **out,
    char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API int weil_secure_wallet_from_api(
    const char *api_key, const char *endpoint, int verify_tls, int require_lock,
    int include_creds, weil_secure_wallet **out,
    char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API int weil_secure_wallet_sign_digest(
    weil_secure_wallet *wallet, const uint8_t digest[32], uint8_t signature[64],
    char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API int weil_secure_wallet_public_key(
    weil_secure_wallet *wallet, uint8_t public_key[65],
    char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API int weil_secure_wallet_status(
    weil_secure_wallet *wallet, weil_secure_status *status,
    char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API int weil_secure_wallet_metadata(
    weil_secure_wallet *wallet, weil_secure_metadata *metadata,
    char error[WEIL_SECURE_ERROR_SIZE]);

WEIL_SECURE_API void weil_secure_wallet_free(weil_secure_wallet *wallet);

WEIL_SECURE_API unsigned int weil_secure_wallet_abi_version(void);

#ifdef __cplusplus
}
#endif
#endif
