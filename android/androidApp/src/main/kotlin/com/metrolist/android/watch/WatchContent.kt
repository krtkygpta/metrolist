/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Color
import coil3.ImageLoader
import coil3.request.ImageRequest
import coil3.request.SuccessResult
import coil3.request.allowHardware
import coil3.toBitmap
import com.metrolist.android.watch.WatchNodeBook.Entry
import com.metrolist.android.watch.WatchNodeBook.Kind
import com.metrolist.android.watch.WatchNodeBook.Node
import com.metrolist.shared.api.YouTubeRepository
import com.metrolist.shared.db.MusicDatabase
import android.util.Log
import com.metrolist.shared.db.entities.toSongItem
import com.metrolist.shared.model.AlbumItem
import com.metrolist.shared.model.HomeSection
import com.metrolist.shared.model.PlaylistItem
import com.metrolist.shared.model.SongItem
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Builds the node tree the watch browses from the same data the app's own screens use.
 *
 * The account, the sign-in, the downloads and the player stay here: only a title, an artist, a colour and a play
 * state ever cross the link. A signed-out or still-loading node answers "not ready" instead of "empty".
 */
internal class WatchContent(
    private val context: Context,
    private val database: MusicDatabase,
    private val repository: YouTubeRepository,
    private val colours: ArtworkColours,
) {
    internal data class Built(
        val root: Node,
        val nodes: List<Node>,
        val kinds: Map<Int, Kind>,
    )

    /** Mood tiles opened from the watch: node id -> browse id. */
    private val childBrowse = HashMap<Int, String>()

    /** Album rows opened from the watch: node id -> album browse id. Its songs load when the watch opens it. */
    private val childAlbums = HashMap<Int, String>()

    private val childTitles = HashMap<Int, String>()

    private val lock = Mutex()

    suspend fun build(): Built =
        lock.withLock {
            childBrowse.clear()
            childAlbums.clear()
            childTitles.clear()
            val dao = database.dao()
            val home = runCatching { repository.getHomePageData() }.getOrNull()
            val feed = ArrayList<HomeSection>(home?.sections.orEmpty())
            // Quick picks is not always on the first page of the feed, and a single browse misses it entirely then: walk
            // at most two continuation pages, and only while the shelf has not shown up.
            var token = home?.continuation
            var pages = 0
            while (token != null && pages < 2 && shelfSongs(feed, QUICK_SHELF, "Quick picks").isEmpty()) {
                val page =
                    runCatching { repository.getHomeSectionsContinuation(token, loadAllPages = false) }.getOrNull()
                        ?: break
                feed += page.first
                token = page.second
                pages++
            }
            val apiQuick = shelfSongs(feed, QUICK_SHELF, "Quick picks")
            // The app's own Quick picks is the API shelf and the local history together, not one or the other: a fresh
            // install has the first, an offline or signed-out one the second. The most played songs of all time are the
            // last resort, so the shelf holds songs instead of an empty list.
            val quick =
                mergeSongs(
                    apiQuick,
                    runCatching { dao.quickPicks().first().map { it.toSongItem() } }.getOrDefault(emptyList()),
                    runCatching {
                        dao.mostPlayedSongs(fromTimeStamp = 0L, limit = WatchProtocol.MAX_ITEMS).first()
                            .map { it.toSongItem() }
                    }.getOrDefault(emptyList()),
                    runCatching { dao.forgottenFavorites().first().map { it.toSongItem() } }.getOrDefault(emptyList()),
                ).take(WatchProtocol.MAX_ITEMS)
            val listen =
                mergeSongs(
                    shelfSongs(feed, LISTEN_SHELF, "Listen again"),
                    runCatching {
                        val monthAgo = System.currentTimeMillis() - 30L * 24L * 60L * 60L * 1000L
                        dao.mostPlayedSongs(fromTimeStamp = monthAgo, limit = WatchProtocol.MAX_ITEMS).first()
                            .map { it.toSongItem() }
                    }.getOrDefault(emptyList()),
                ).take(WatchProtocol.MAX_ITEMS)
            // One line per rebuild: when the watch shows an empty shelf, logcat says whether the phone had rows to send.
            Log.i(
                WATCH_LOG,
                "feed ${feed.size} sections, quick ${apiQuick.size}/${quick.size}, listen ${listen.size}" +
                    if (quick.isEmpty()) {
                        ": " + feed.joinToString { "${it.title}|${it.browseId}|${it.items.size}" }
                    } else {
                        ""
                    },
            )
            val downloads =
                runCatching { dao.downloadedSongsByCreateDateAsc().first().map { it.toSongItem() } }
                    .getOrDefault(emptyList())
            val library =
                runCatching { dao.songsByRowIdAsc().first().map { it.toSongItem() } }.getOrDefault(emptyList())

            // No home data and nothing local means signed out or still loading: the watch says so instead of "empty".
            val notReady = home == null && quick.isEmpty() && listen.isEmpty()
            val quickNode = Node(ID_QUICK, "Quick picks", Kind.SONGS, tracks(quick, "quick"), notReady)
            val listenNode = Node(ID_LISTEN, "Listen again", Kind.SONGS, tracks(listen, "listen"), notReady)
            val downloadsNode =
                Node(
                    ID_DOWNLOADS,
                    "Downloads",
                    Kind.FOLDER,
                    listOf(folder("Songs", downloads.size, ID_DOWNLOAD_SONGS, "downloads")),
                )
            val downloadSongs = Node(ID_DOWNLOAD_SONGS, "Downloaded songs", Kind.SONGS, tracks(downloads, "downloads"))
            val libraryNode =
                Node(
                    ID_LIBRARY,
                    "Library",
                    Kind.FOLDER,
                    listOf(folder("Songs", library.size, ID_LIBRARY_SONGS, "library")),
                )
            val librarySongs = Node(ID_LIBRARY_SONGS, "Library songs", Kind.SONGS, tracks(library, "library"))

            val (exploreRoot, exploreNodes) = exploreNode()
            val rootEntries =
                buildList {
                    // Always offered, even empty: it is the app's first shelf, and a shelf that vanished from the
                    // watch's menu whenever its rows were not ready read as a missing feature.
                    add(folder("Quick picks", quick.size, ID_QUICK, "quick"))
                    if (listen.isNotEmpty()) add(folder("Listen again", listen.size, ID_LISTEN, "listen"))
                    if (downloads.isNotEmpty()) add(folder("Downloads", downloads.size, ID_DOWNLOADS, "downloads"))
                    if (library.isNotEmpty()) add(folder("Library", library.size, ID_LIBRARY, "library"))
                    if (exploreRoot.entries.isNotEmpty()) add(folder("Explore", exploreRoot.entries.size, ID_EXPLORE, "explore"))
                }.toMutableList()
            val seenTitles = rootEntries.mapTo(HashSet()) { it.title }
            val sectionNodes = ArrayList<Node>()
            val sectionKinds = HashMap<Int, Kind>()
            var dynamicId = DYNAMIC_FIRST
                            for (section in feed) {
                if (rootEntries.size >= WatchProtocol.MAX_ROOT || dynamicId > WatchNodeBook.MAX_NODE) break
                if (section.title in seenTitles) continue
                val songs =
                    section.items
                        .filterIsInstance<SongItem>()
                        .distinctBy { it.id }
                        .take(WatchProtocol.MAX_ITEMS)
                if (songs.isNotEmpty()) {
                    val seed = "section-$dynamicId"
                    rootEntries.add(folder(section.title, songs.size, dynamicId, seed))
                    sectionNodes.add(Node(dynamicId, section.title, Kind.SONGS, tracks(songs, seed)))
                    sectionKinds[dynamicId] = Kind.SONGS
                    seenTitles.add(section.title)
                    dynamicId++
                    continue
                }

                // A shelf of albums (New releases, Trending, ...): each album opens its own track list, so a tap on a
                // song there plays the album from that song. The albums' covers are not read (the watch's list does
                // not tint rows): twenty artwork loads would only delay the shelf.
                val albums =
                    section.items
                        .filterIsInstance<AlbumItem>()
                        .distinctBy { it.id }
                        .take(WatchProtocol.MAX_ITEMS)
                if (albums.isNotEmpty()) {
                    val folderNode = dynamicId
                    val after = folderNode + albums.size
                    if (after > WatchNodeBook.MAX_NODE) break
                    val entries = ArrayList<Entry>(albums.size)
                    for ((i, album) in albums.withIndex()) {
                        val id = folderNode + 1 + i
                        childAlbums[id] = album.id
                        childTitles[id] = album.title
                        entries +=
                            Entry.Folder(
                                title = album.title,
                                artist = album.artists.joinToString(", ") { it.name },
                                colour = 0,
                                node = id,
                            )
                    }
                    rootEntries.add(folder(section.title, albums.size, folderNode, "section-$folderNode"))
                    sectionNodes.add(Node(folderNode, section.title, Kind.FOLDER, entries))
                    sectionKinds[folderNode] = Kind.FOLDER
                    seenTitles.add(section.title)
                    dynamicId = after + 1
                    continue
                }

                val browseId = section.browseId
                if (browseId.isNullOrBlank()) continue
                val folderNode = dynamicId
                val childNode = dynamicId + 1
                if (folderNode > WatchNodeBook.MAX_NODE || childNode > WatchNodeBook.MAX_NODE) break
                childBrowse[childNode] = browseId
                childTitles[childNode] = section.title
                val count = if (section.items.isEmpty()) 0 else section.items.size
                val folderEntry = folder(section.title, count, folderNode, "section-$folderNode")
                rootEntries.add(folderEntry)
                sectionNodes.add(
                    Node(
                        folderNode,
                        section.title,
                        Kind.FOLDER,
                        listOf(folderEntry.copy(node = childNode)),
                    ),
                )
                sectionKinds[folderNode] = Kind.FOLDER
                seenTitles.add(section.title)
                sectionNodes.add(Node(childNode, section.title, Kind.SONGS, tracks(emptyList(), "section-$childNode")))
                dynamicId += 2
            }
            val root = Node(WatchProtocol.ROOT_NODE, "Metrolist", Kind.FOLDER, rootEntries)
            Built(
                root = root,
                nodes =
                    listOf(quickNode, listenNode, downloadsNode, downloadSongs, libraryNode, librarySongs) +
                        exploreNodes + sectionNodes,
                kinds =
                    mapOf(
                        ID_QUICK to Kind.SONGS,
                        ID_LISTEN to Kind.SONGS,
                        ID_DOWNLOADS to Kind.FOLDER,
                        ID_DOWNLOAD_SONGS to Kind.SONGS,
                        ID_LIBRARY to Kind.FOLDER,
                        ID_LIBRARY_SONGS to Kind.SONGS,
                        ID_EXPLORE to Kind.FOLDER,
                    ) + sectionKinds,
            )
        }

    /** The playing track's dominant artwork colour (RGB565) for the watch's player glow. */
    suspend fun colourOf(song: SongItem): Int = colours.of(song.thumbnail, "now:${song.id}")

    /**
     * The phone's best matches for a name typed on the watch, as the watch's list. A `SongItem` from the search goes
     * straight into the player, so choosing one plays the phone's own result and the rest stay queued behind it. No
     * artwork is fetched here: the list page does not tint rows, and five covers would only delay the answer.
     */
    suspend fun search(query: String): Node? {
        val results = runCatching { repository.search(query.trim()) }.getOrNull() ?: return null
        val songs =
            results
                .filterIsInstance<SongItem>()
                .distinctBy { it.id }
                .take(WatchProtocol.MAX_SEARCH)
        return Node(
            WatchProtocol.SEARCH_NODE,
            "Results",
            Kind.SONGS,
            songs.map { song ->
                Entry.Track(
                    title = song.customTitle ?: song.title,
                    artist = song.artists.joinToString(", ") { it.name },
                    colour = 0,
                    song = song,
                )
            },
        )
    }

    /**
     * Loads a node the root advertised but that is not built yet: an album's tracks, or a mood shelf's songs. `null`
     * when it cannot be read.
     */
    suspend fun loadChild(nodeId: Int): Node? =
        lock.withLock {
            val albumId = childAlbums[nodeId]
            if (albumId != null) {
                val page = runCatching { repository.getAlbum(albumId) }.getOrNull() ?: return@withLock null
                // The album page carries its tracks in album order, which is the order the watch plays down.
                val songs = page.songs.distinctBy { it.id }.take(WatchProtocol.MAX_ITEMS)
                return@withLock Node(
                    id = nodeId,
                    title = page.album.title.ifBlank { childTitles[nodeId] ?: "Album" },
                    kind = Kind.SONGS,
                    entries = tracks(songs, "album-$nodeId"),
                    fromAlbum = true,
                )
            }
            val browseId = childBrowse[nodeId] ?: return@withLock null
            val title = childTitles[nodeId] ?: "Explore"
            val page = runCatching { repository.getBrowsePageData(browseId) }.getOrNull() ?: return null
            val songs =
                page.sections
                    .flatMap { it.items }
                    .filterIsInstance<SongItem>()
                    .distinctBy { it.id }
                    .take(WatchProtocol.MAX_ITEMS)
            Node(nodeId, title, Kind.SONGS, tracks(songs, "mood-$nodeId"))
        }

    private suspend fun exploreNode(): Pair<Node, List<Node>> {
        val page = runCatching { repository.getHomePageData(browseId = "FEmusic_moods_and_genres") }.getOrNull()
        val tiles =
            page?.sections.orEmpty()
                .flatMap { it.items }
                .filterIsInstance<PlaylistItem>()
                .distinctBy { it.id }
                .take(WatchProtocol.MAX_ITEMS)
        var next = CHILD_FIRST
        val entries = ArrayList<Entry>(tiles.size)
        for (tile in tiles) {
            val id = next++
            if (id > WatchNodeBook.MAX_NODE) break
            childBrowse[id] = tile.id
            childTitles[id] = tile.title
            entries +=
                Entry.Folder(
                    title = tile.title,
                    artist = tile.songCount.orEmpty(),
                    colour = colours.of(tile.thumbnail, "mood:${tile.id}"),
                    node = id,
                )
            // The songs themselves are loaded from the browse id when the watch opens the tile.
        }
        return Node(ID_EXPLORE, "Explore", Kind.FOLDER, entries) to emptyList()
    }

    private suspend fun tracks(
        songs: List<SongItem>,
        seed: String,
    ): List<Entry> =
        songs
            .distinctBy { it.id }
            .take(WatchProtocol.MAX_ITEMS)
            .mapIndexed { index, song ->
                Entry.Track(
                    title = song.customTitle ?: song.title,
                    artist = song.artists.joinToString(", ") { it.name },
                    colour = colours.of(song.thumbnail, "$seed:${song.id}:$index"),
                    song = song,
                )
            }

    private fun folder(
        title: String,
        count: Int,
        node: Int,
        seed: String,
    ): Entry.Folder =
        Entry.Folder(
            title = title,
            artist = if (count > 0) "$count" else "",
            colour = fallbackColour("$seed:$title"),
            node = node,
        )

    /**
     * The songs of a home shelf: by its browse id, or by its title (a localized home response does not always carry
     * the browse id, and the app's own Quick picks shelf is matched by name too).
     */
    private fun shelfSongs(
        sections: List<HomeSection>?,
        browseId: String,
        title: String,
    ): List<SongItem> =
        sections.orEmpty()
            .filter { it.browseId?.startsWith(browseId) == true || it.title.equals(title, ignoreCase = true) }
            .flatMap { it.items.filterIsInstance<SongItem>() }
            .distinctBy { it.id }

    private fun mergeSongs(vararg lists: List<SongItem>): List<SongItem> = lists.flatMap { it }.distinctBy { it.id }

    private fun fallbackColour(seed: String): Int {
        val hue = (seed.hashCode() and 0x7FFFFFFF) % 360
        return WatchProtocol.rgb565(Color.HSVToColor(floatArrayOf(hue.toFloat(), 0.5f, 0.72f)))
    }

    companion object {
        /** The link service's own log tag, so one logcat filter shows the whole watch path. */
        private const val WATCH_LOG = "MetrolistWatch"
        private const val QUICK_SHELF = "FEmusic_quickpicks"
        private const val LISTEN_SHELF = "FEmusic_listen_again"

        const val ID_QUICK = 1
        const val ID_LISTEN = 2
        const val ID_DOWNLOADS = 3
        const val ID_DOWNLOAD_SONGS = 6
        const val ID_LIBRARY = 4
        const val ID_LIBRARY_SONGS = 7
        const val ID_EXPLORE = 5
        private const val CHILD_FIRST = 8
        private const val DYNAMIC_FIRST = 64
    }
}

/**
 * The dominant colour of an item's artwork, RGB565, cached per URL. It never leaves the phone as an image: the watch
 * only gets the 16-bit colour for its row, or for the player's glow.
 */
internal class ArtworkColours(
    private val context: Context,
    private val imageLoader: ImageLoader,
) {
    private companion object {
        /** 4 bits per channel: 4096 buckets, coarse enough that one sleeve lands in a few of them. */
        const val BUCKETS = 4096
    }

    private val cache = LinkedHashMap<String, Int>()
    private val lock = Mutex()

    suspend fun of(
        url: String?,
        seed: String,
    ): Int {
        if (url.isNullOrBlank()) return fallback(seed)
        lock.withLock { cache[url] }?.let { return it }
        val colour =
            runCatching {
                val request =
                    ImageRequest
                        .Builder(context)
                        .data(url)
                        .allowHardware(false)
                        .build()
                val result = imageLoader.execute(request) as? SuccessResult ?: return@runCatching fallback(seed)
                dominant(result.image.toBitmap())
            }.getOrDefault(fallback(seed))
        lock.withLock {
            cache[url] = colour
            while (cache.size > 400) cache.remove(cache.keys.first())
        }
        return colour
    }

    /**
     * The artwork's dominant colour: the heaviest 4-bit-per-channel bucket of a 24x24 copy. Pixels are weighted by how
     * colourful they are, so a grey or white background does not outvote the sleeve's real colour; transparent,
     * near-black and blown-out pixels are skipped. The bucket's own average is returned, not its centre.
     */
    private fun dominant(bitmap: Bitmap): Int {
        val scaled =
            if (bitmap.width > 24 || bitmap.height > 24) {
                Bitmap.createScaledBitmap(bitmap, 24, 24, false)
            } else {
                bitmap
            }
        val pixels = IntArray(scaled.width * scaled.height)
        scaled.getPixels(pixels, 0, scaled.width, 0, 0, scaled.width, scaled.height)
        if (scaled !== bitmap) scaled.recycle()
        val weight = IntArray(BUCKETS)
        val red = IntArray(BUCKETS)
        val green = IntArray(BUCKETS)
        val blue = IntArray(BUCKETS)
        val count = IntArray(BUCKETS)
        for (pixel in pixels) {
            if ((pixel ushr 24) < 128) continue
            val r = (pixel shr 16) and 0xFF
            val g = (pixel shr 8) and 0xFF
            val b = pixel and 0xFF
            // Skip near-black and near-white edges: they would wash the glow out on the watch.
            val luma = (r * 299 + g * 587 + b * 114) / 1000
            if (luma < 24 || luma > 240) continue
            val high = maxOf(r, g, b)
            val low = minOf(r, g, b)
            val saturation = if (high == 0) 0 else (high - low) * 255 / high
            val key = ((r shr 4) shl 8) or ((g shr 4) shl 4) or (b shr 4)
            // A fully saturated pixel counts up to 32 times a grey one, so the colour of the art wins over its paper.
            weight[key] += 1 + saturation / 8
            red[key] += r
            green[key] += g
            blue[key] += b
            count[key]++
        }
        var best = -1
        for (key in 0 until BUCKETS) {
            if (count[key] == 0) continue
            if (best < 0 || weight[key] > weight[best]) best = key
        }
        if (best < 0) return WatchProtocol.rgb565(Color.HSVToColor(floatArrayOf(210f, 0.4f, 0.7f)))
        val argb =
            Color.rgb(
                red[best] / count[best],
                green[best] / count[best],
                blue[best] / count[best],
            )
        return WatchProtocol.rgb565(argb)
    }

    private fun fallback(seed: String): Int {
        val hue = (seed.hashCode() and 0x7FFFFFFF) % 360
        return WatchProtocol.rgb565(Color.HSVToColor(floatArrayOf(hue.toFloat(), 0.5f, 0.72f)))
    }
}
