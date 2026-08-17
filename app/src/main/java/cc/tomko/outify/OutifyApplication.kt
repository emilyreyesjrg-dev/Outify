package cc.tomko.outify

import android.app.Application
import android.content.Intent
import android.widget.Toast
import androidx.core.content.ContextCompat
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.glance.GlanceId
import androidx.media3.common.util.UnstableApi
import cc.tomko.outify.core.Spirc.SpircWrapper
import cc.tomko.outify.core.spirc.SpircController
import cc.tomko.outify.data.database.AppDatabase
import cc.tomko.outify.services.PlaybackService
import cc.tomko.outify.ui.viewmodel.detail.DetailViewModelStore
import cc.tomko.outify.ui.viewmodel.detail.setDetailViewModelStore
import cc.tomko.outify.utils.ExceptionCollector
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import javax.inject.Inject

const val ALBUM_COVER_URL: String = "https://i.scdn.co/image/"
fun widgetMediaPreference(id: GlanceId) =
    stringPreferencesKey("widget_media_$id")

@HiltAndroidApp
class OutifyApplication : Application() {
    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    val database: AppDatabase by lazy { AppDatabase.getInstance(this) }

    @Inject
    lateinit var spircController: SpircController

    @Inject
    lateinit var spircWrapper: SpircWrapper

    @Inject
    lateinit var detailViewModelStore: DetailViewModelStore

    @Inject
    lateinit var exceptionCollector: ExceptionCollector

    @UnstableApi
    override fun onCreate() {
        super.onCreate()
        exceptionCollector.install()

        setDetailViewModelStore(detailViewModelStore)

        System.loadLibrary("librespot_ffi")

        val spotifySecret = BuildConfig.SPOTIFY_CLIENT_SECRET
        val spotifyId = BuildConfig.SPOTIFY_CLIENT_ID

        if(spotifySecret.isEmpty() || spotifyId.isEmpty()) {
            Toast.makeText(this, "No Spotify credentials were supplied during build", Toast.LENGTH_LONG).show()
            throw Exception("No Spotify credentials were supplied during build! spotify.playback.clientId is${if(spotifyId.isEmpty()) "" else " not"} empty; spotify.playback.clientSecret is${if(spotifySecret.isEmpty()) "" else " not"} empty")
        }

        appScope.launch {
            LibrespotFfi.libInit(applicationContext, spotifyId, spotifySecret)

            val intent = Intent(this@OutifyApplication, PlaybackService::class.java)
            ContextCompat.startForegroundService(this@OutifyApplication, intent)

            spircController.start()
            spircWrapper.setRestartCallback { spircController.restart() }
        }
    }
}