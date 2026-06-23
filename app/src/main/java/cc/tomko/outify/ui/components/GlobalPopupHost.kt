package cc.tomko.outify.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.navigation3.runtime.NavBackStack
import androidx.navigation3.runtime.NavKey
import cc.tomko.outify.core.model.OutifyUri
import cc.tomko.outify.core.model.Track
import cc.tomko.outify.core.model.toOutifyUri
import cc.tomko.outify.ui.GlobalPopupController
import cc.tomko.outify.ui.PopupSpec
import cc.tomko.outify.ui.components.bottomsheet.AddToPlaylistBottomSheet
import cc.tomko.outify.ui.components.bottomsheet.AddToWidgetBottomSheet
import cc.tomko.outify.ui.components.bottomsheet.ArtistInfoBottomSheet
import cc.tomko.outify.ui.components.bottomsheet.AuthResultBottomSheet
import cc.tomko.outify.ui.components.bottomsheet.CreatePlaylistBottomSheet
import cc.tomko.outify.ui.components.bottomsheet.PlaybackDevicesBottomSheet
import cc.tomko.outify.ui.components.bottomsheet.PlaylistInfoBottomSheet
import cc.tomko.outify.ui.components.bottomsheet.TrackInfoBottomSheet
import cc.tomko.outify.ui.components.navigation.Route
import cc.tomko.outify.ui.viewmodel.bottomsheet.AddToPlaylistViewModel
import cc.tomko.outify.ui.viewmodel.bottomsheet.AddToWidgetViewModel
import cc.tomko.outify.ui.viewmodel.bottomsheet.CreatePlaylistViewModel
import cc.tomko.outify.ui.viewmodel.bottomsheet.PlaybackDevicesViewModel
import kotlinx.coroutines.launch

@Composable
fun GlobalPopupHost(
    backStack: NavBackStack<NavKey>,
    addToQueue: (OutifyUri) -> Unit,
    playNext: (OutifyUri) -> Unit,
    startRadio: (Track) -> Unit,
    openRadio: (Track) -> Unit,
    addToPlaylist: (Track) -> Unit,
    toggleLike: (OutifyUri) -> Unit,

    addToPlaylistViewModel: AddToPlaylistViewModel,
    createPlaylistViewModel: CreatePlaylistViewModel,
    playbackDevicesViewModel: PlaybackDevicesViewModel,
    addToWidgetViewModel: AddToWidgetViewModel,
) {
    val popups by GlobalPopupController.popups.collectAsState()
    val scope = rememberCoroutineScope()

    popups.forEach { popup ->
        when (popup) {
            is PopupSpec.TrackInfo -> {
                TrackInfoBottomSheet(
                    track = popup.track,
                    likedTrackIndex = popup.likedTrackIndex,
                    isLiked = popup.isLiked,
                    onDismiss = { GlobalPopupController.dismiss(popup.id) },
                    onArtworkClick = {
                        val albumUri = popup.track.album?.uri
                        if (albumUri != null) {
                            backStack.add(Route.AlbumScreen(albumUri))
                        } else {
                            backStack.add(Route.TrackScreen(popup.track.uri))
                        }
                        popup.action?.invoke()
                    },
                    onArtistClick = { artist ->
                        backStack.add(Route.ArtistScreen(artist.uri))
                        popup.action?.invoke()
                    },
                    onOpenAlbum = {
                        val albumUri = popup.track.album?.uri
                        if (albumUri != null) {
                            backStack.add(Route.AlbumScreen(albumUri))
                        } else {
                            backStack.add(Route.TrackScreen(popup.track.uri))
                        }
                        popup.action?.invoke()
                    },
                    onOpenArtist = {
                        backStack.add(Route.ArtistScreen(popup.track.artists.first().uri))
                        popup.action?.invoke()
                    },
                    onAddToQueue = { addToQueue(popup.track.toOutifyUri()) },
                    onPlayNext = { playNext(popup.track.toOutifyUri()) },
                    onAddToPlaylist = { addToPlaylist(popup.track) },
                    onToggleLike = { toggleLike(popup.track.toOutifyUri()) },
                    onStartRadio = { startRadio(popup.track) },
                    onOpenRadio = {
                        openRadio(popup.track)
                        popup.action?.invoke()
                    },
                    onScrollToLiked = {
                        scope.launch {
                            popup.action?.invoke()
                            backStack.add(
                                Route.LikedScreen(
                                    scrollToIndex = popup.likedTrackIndex ?: -1
                                )
                            )
                            GlobalPopupController.dismiss(popup.id)
                        }
                    },
                    onAddToWidget = {
                        scope.launch {
                            GlobalPopupController.show(PopupSpec.AddToWidgetInfo(popup.track))
                        }
                    }
                )
            }

            is PopupSpec.AddToWidgetInfo -> {
                AddToWidgetBottomSheet(
                    track = popup.track,
                    viewModel = addToWidgetViewModel,
                    onDismiss = { GlobalPopupController.dismiss(popup.id) },
                )
            }

            is PopupSpec.PlaylistInfo -> {
                PlaylistInfoBottomSheet(
                    playlist = popup.playlist,
                    artworkUrl = popup.artworkUrl,
                    onDismiss = { GlobalPopupController.dismiss(popup.id) },
                    onOpenPlaylist = {
                        backStack.add(Route.PlaylistScreen(popup.playlist.uri))
                        GlobalPopupController.dismiss(popup.id)
                    },
                    onOpenCreator = {
                        backStack.add(
                            Route.ProfileScreen(
                                popup.playlist.uri.substringBefore(":").let { "spotify:user:$it" })
                        )
                        GlobalPopupController.dismiss(popup.id)
                    },

                    onAddToQueue = { addToQueue(popup.playlist.toOutifyUri()) },
                    onPlayNext = { playNext(popup.playlist.toOutifyUri()) },
                    onToggleLike = { toggleLike(popup.playlist.toOutifyUri()) },
                )
            }

            is PopupSpec.ArtistInfo -> {
                ArtistInfoBottomSheet(
                    artist = popup.artist,
                    isSaved = popup.isSaved,
                    onDismiss = { GlobalPopupController.dismiss(popup.id) },
                    onToggleSave = {
                        popup.onToggleSave?.invoke()
                        GlobalPopupController.dismiss(popup.id)
                    },
                    onAddToQueue = { addToQueue(popup.artist.toOutifyUri()) },
                    onPlayNext = { playNext(popup.artist.toOutifyUri()) },
                    onOpenArtist = {
                        backStack.add(Route.ArtistScreen(popup.artist.uri))
                        GlobalPopupController.dismiss(popup.id)
                    },
                )
            }

            is PopupSpec.AuthResult -> {
                AuthResultBottomSheet(
                    isSuccess = popup.isSuccess,
                    message = popup.message,
                    errorDetails = popup.errorDetails,
                    onDismiss = {
                        GlobalPopupController.dismiss(popup.id)
                        popup.onDismiss?.invoke()
                    },
                )
            }

            is PopupSpec.AddToPlaylist -> {
                AddToPlaylistBottomSheet(
                    viewModel = addToPlaylistViewModel,
                    tracks = popup.tracks,
                    onDismiss = {
                        GlobalPopupController.dismiss(popup.id)
                    }
                )
            }

            is PopupSpec.CreatePlaylist -> {
                CreatePlaylistBottomSheet(
                    viewModel = createPlaylistViewModel,
                    onDismiss = {
                        GlobalPopupController.dismiss(popup.id)
                    },
                    onCreated = { },
                )
            }

            is PopupSpec.ModifyPlaylist -> {
                CreatePlaylistBottomSheet(
                    viewModel = createPlaylistViewModel,
                    onDismiss = {
                        GlobalPopupController.dismiss(popup.id)
                    },
                    onCreated = { },
                    playlistId = popup.playlistId,
                    initialName = popup.name,
                    initialDescription = popup.description,
                    initialPublic = popup.public,
                    initialCollaborative = popup.collaborative,
                )
            }

            is PopupSpec.PlaybackDevices -> {
                PlaybackDevicesBottomSheet(
                    viewModel = playbackDevicesViewModel,
                    onDismiss = {
                        GlobalPopupController.dismiss(popup.id)
                    }
                )
            }

            is PopupSpec.CreateFolder,
            is PopupSpec.EditFolder -> {
                // Handled locally in LibraryScreen
            }
        }
    }
}
