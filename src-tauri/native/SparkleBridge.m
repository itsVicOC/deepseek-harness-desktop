#import <Cocoa/Cocoa.h>
#import <Sparkle/Sparkle.h>
#include <stdbool.h>

static SPUStandardUpdaterController *dshUpdaterController;

bool dsh_sparkle_available(void) {
    return true;
}

bool dsh_sparkle_check_for_updates(const char *feed_url) {
    if (![NSThread isMainThread]) return false;
    if (feed_url == NULL) return false;
    if (dshUpdaterController == nil) {
        dshUpdaterController = [[SPUStandardUpdaterController alloc]
            initWithStartingUpdater:true
            updaterDelegate:nil
            userDriverDelegate:nil];
    }
    NSString *feedString = [NSString stringWithUTF8String:feed_url];
    NSURL *feedURL = [NSURL URLWithString:feedString];
    if (feedURL == nil || ![feedURL.scheme isEqualToString:@"https"]) return false;
    dshUpdaterController.updater.feedURL = feedURL;
    [dshUpdaterController checkForUpdates:nil];
    return true;
}
