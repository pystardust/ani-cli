/*
 * aesgcm - tiny AES-256-GCM helper for ani-cli
 *
 * Usage: aesgcm <hex_key> <hex_iv> < plaintext > ciphertext_and_tag
 *
 * Reads plaintext from stdin, encrypts with AES-256-GCM.
 * Outputs: ciphertext (variable length) followed by 16-byte tag to stdout.
 * Exit 0 on success, 1 on failure.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <openssl/evp.h>

static int hex2bin(const char *hex, unsigned char *out, size_t out_len) {
    size_t hex_len = strlen(hex);
    if (hex_len % 2 != 0 || hex_len / 2 > out_len)
        return -1;
    for (size_t i = 0; i < hex_len; i += 2) {
        unsigned int byte;
        if (sscanf(hex + i, "%2x", &byte) != 1)
            return -1;
        out[i / 2] = (unsigned char)byte;
    }
    return 0;
}

int main(int argc, char *argv[]) {
    if (argc != 3) {
        fprintf(stderr, "Usage: aesgcm <hex_key> <hex_iv>\n");
        return 1;
    }

    unsigned char key[32], iv[12];
    if (hex2bin(argv[1], key, sizeof(key)) < 0) {
        fprintf(stderr, "Invalid key\n");
        return 1;
    }
    if (hex2bin(argv[2], iv, sizeof(iv)) < 0) {
        fprintf(stderr, "Invalid IV\n");
        return 1;
    }

    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    if (!ctx) return 1;

    if (EVP_EncryptInit_ex(ctx, EVP_aes_256_gcm(), NULL, NULL, NULL) != 1)
        goto err;
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, sizeof(iv), NULL) != 1)
        goto err;
    if (EVP_EncryptInit_ex(ctx, NULL, NULL, key, iv) != 1)
        goto err;

    /* Read all plaintext from stdin */
    unsigned char pt_buf[65536];
    unsigned char ct_buf[sizeof(pt_buf) + 16];
    int total_ct = 0;
    int n;
    while ((n = fread(pt_buf, 1, sizeof(pt_buf), stdin)) > 0) {
        int out_len = 0;
        if (EVP_EncryptUpdate(ctx, ct_buf + total_ct, &out_len, pt_buf, n) != 1)
            goto err;
        total_ct += out_len;
    }
    {
        int out_len = 0;
        if (EVP_EncryptFinal_ex(ctx, ct_buf + total_ct, &out_len) != 1)
            goto err;
        total_ct += out_len;
    }

    /* Write ciphertext */
    fwrite(ct_buf, 1, total_ct, stdout);

    /* Get and write tag */
    unsigned char tag[16];
    if (EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, sizeof(tag), tag) != 1)
        goto err;
    fwrite(tag, 1, sizeof(tag), stdout);

    EVP_CIPHER_CTX_free(ctx);
    return 0;

err:
    EVP_CIPHER_CTX_free(ctx);
    fprintf(stderr, "AES-256-GCM encryption failed\n");
    return 1;
}
