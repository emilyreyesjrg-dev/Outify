package cc.tomko.outify.core

import javax.inject.Inject
import javax.inject.Singleton

@Singleton
class AuthCallbackServerManager @Inject constructor() {
    @Volatile
    private var server: AuthCallbackServer? = null

    fun start(onCodeReceived: (code: String, state: String?) -> Unit) {
        synchronized(this) {
            server?.stop()
            server = AuthCallbackServer(onCodeReceived = onCodeReceived).apply { start() }
        }
    }

    fun stop() {
        synchronized(this) {
            server?.stop()
            server = null
        }
    }
}
