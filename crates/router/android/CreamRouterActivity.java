package dev.creamui.router;

import android.app.NativeActivity;
import android.content.Intent;

/** A single NativeActivity host exposing reused-Activity deep links to Rust. */
public class CreamRouterActivity extends NativeActivity {
    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
    }
}
