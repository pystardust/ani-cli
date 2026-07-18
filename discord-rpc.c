#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <time.h>
#include <errno.h>

#define DISCORD_IPC_PATH "/run/user/%d/discord-ipc-0"
#define HANDSHAKE "{\"v\":1,\"client_id\":\"1528138773341274122\"}"
#define BUF_SIZE 4096

static int send_ipc(int fd, int op, const char *data, size_t len) {
    unsigned char header[8];
    header[0] = op & 0xFF;
    header[1] = (op >> 8) & 0xFF;
    unsigned int ulen = (unsigned int)len;
    header[2] = ulen & 0xFF;
    header[3] = (ulen >> 8) & 0xFF;
    header[4] = (ulen >> 16) & 0xFF;
    header[5] = (ulen >> 24) & 0xFF;
    header[6] = 0;
    header[7] = 0;
    if (write(fd, header, 8) < 0) return -1;
    if (len > 0 && write(fd, data, len) < 0) return -1;
    return 0;
}

static char *recv_ipc(int fd) {
    unsigned char header[8];
    ssize_t n = read(fd, header, 8);
    if (n < 8) return NULL;
    unsigned int len = header[2] | (header[3] << 8) | (header[4] << 16) | (header[5] << 24);
    if (len == 0 || len > BUF_SIZE) return NULL;
    char *buf = malloc(len + 1);
    if (!buf) return NULL;
    ssize_t total = 0;
    while ((size_t)total < len) {
        n = read(fd, buf + total, len - total);
        if (n <= 0) { free(buf); return NULL; }
        total += n;
    }
    buf[len] = '\0';
    return buf;
}

static int connect_discord(void) {
    char path[64];
    snprintf(path, sizeof(path), DISCORD_IPC_PATH, getuid());
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) return -1;
    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    strncpy(addr.sun_path, path, sizeof(addr.sun_path) - 1);
    if (connect(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        close(fd);
        return -1;
    }
    return fd;
}

int main(int argc, char *argv[]) {
    if (argc < 2) {
        fprintf(stderr, "Usage: %s <playing|stopped> [title] [detail] [start_timestamp]\n", argv[0]);
        return 1;
    }
    const char *action = argv[1];
    int fd = connect_discord();
    if (fd < 0) return 1;

    /* handshake - opcode 0 (DISPATCH) */
    send_ipc(fd, 0, HANDSHAKE, strlen(HANDSHAKE));
    char *resp = recv_ipc(fd);
    free(resp);

    if (strcmp(action, "stopped") == 0) {
        /* clear activity - opcode 1 (FRAME) */
        const char *clear = "{\"cmd\":\"SET_ACTIVITY\",\"args\":{\"pid\":0,\"activity\":null},\"nonce\":\"1\"}";
        send_ipc(fd, 1, clear, strlen(clear));
        free(recv_ipc(fd));
        close(fd);
        return 0;
    }

    if (strcmp(action, "playing") == 0 && argc >= 3) {
        const char *title = argv[2];
        const char *detail = argc >= 4 ? argv[3] : "";
        long start_ts = 0;
        if (argc >= 5) start_ts = atol(argv[4]);

        char payload[2048];
        if (start_ts > 0) {
            snprintf(payload, sizeof(payload),
                "{\"cmd\":\"SET_ACTIVITY\",\"args\":{\"pid\":0,\"activity\":{\"name\":\"ani-cli\",\"type\":3,"
                "\"details\":\"%s\",\"state\":\"%s\",\"timestamps\":{\"start\":%ld},"
                "\"buttons\":[{\"label\":\"Watch on anidb.app\",\"url\":\"https://anidb.app\"}]},"
                "\"nonce\":\"1\"}}",
                title, detail, start_ts);
        } else {
            snprintf(payload, sizeof(payload),
                "{\"cmd\":\"SET_ACTIVITY\",\"args\":{\"pid\":0,\"activity\":{\"name\":\"ani-cli\",\"type\":3,"
                "\"details\":\"%s\",\"state\":\"%s\","
                "\"buttons\":[{\"label\":\"Watch on anidb.app\",\"url\":\"https://anidb.app\"}]},"
                "\"nonce\":\"1\"}}",
                title, detail);
        }
        /* opcode 1 (FRAME) */
        send_ipc(fd, 1, payload, strlen(payload));
        free(recv_ipc(fd));
    }

    close(fd);
    return 0;
}
