#include <stdbool.h>

bool dsh_sparkle_available(void) {
    return false;
}

bool dsh_sparkle_check_for_updates(const char *feed_url) {
    (void)feed_url;
    return false;
}
