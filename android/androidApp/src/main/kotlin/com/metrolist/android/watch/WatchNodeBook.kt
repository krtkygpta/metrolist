/*
 * Metrolist Project (C) 2026
 * Licensed under GPL-3.0 | See git history for contributors
 */

package com.metrolist.android.watch

import com.metrolist.shared.model.SongItem

/**
 * The node tree the watch browses. Node `0` is the root categories; every other node is a song list or a folder,
 * stable while the app process lives and never more than [MAX_NODE].
 *
 * The tree is replaced as a whole after a rebuild so a frame is never sent from a half-built book.
 */
internal class WatchNodeBook {
    internal enum class Kind(
        val wire: Int,
    ) {
        FOLDER(WatchProtocol.KIND_FOLDER),
        SONGS(WatchProtocol.KIND_SONGS),
    }

    internal sealed interface Entry {
        val title: String
        val artist: String
        /** RGB565. */
        val colour: Int

        data class Track(
            override val title: String,
            override val artist: String,
            override val colour: Int,
            val song: SongItem,
        ) : Entry

        data class Folder(
            override val title: String,
            override val artist: String,
            override val colour: Int,
            val node: Int,
        ) : Entry
    }

    internal class Node(
        val id: Int,
        val title: String,
        val kind: Kind,
        val entries: List<Entry>,
        /** The phone could not read the data yet (signed out or still loading): answer "not ready", not "empty". */
        val notReady: Boolean = false,
    ) {
        /** The watch keeps the first 20 rows of any list. */
        val shown: List<Entry> get() = entries.take(WatchProtocol.MAX_ITEMS)
    }

    @Volatile
    private var rootNode: Node? = null

    @Volatile
    private var nodes: Map<Int, Node> = emptyMap()

    fun root(): Node? = rootNode

    fun node(id: Int): Node? = nodes[id]

    @Synchronized
    fun replace(
        root: Node,
        nodes: List<Node>,
    ) {
        this.rootNode = root
        this.nodes = nodes.associateBy { it.id }
    }

    @Synchronized
    fun put(node: Node) {
        if (node.id == WatchProtocol.ROOT_NODE) {
            rootNode = node
        } else {
            nodes = nodes + (node.id to node)
        }
    }

    @Synchronized
    fun clear() {
        rootNode = null
        nodes = emptyMap()
    }

    companion object {
        const val MAX_NODE = 200

        /** The root categories, as the watch's `C` frame wants them. */
        fun rootCategories(
            root: Node,
            kinds: Map<Int, Kind>,
        ): List<WatchProtocol.Category> =
            WatchProtocol.MAX_ROOT.let { limit ->
                root.entries.take(limit).mapIndexedNotNull { _, entry ->
                    val folder = entry as? Entry.Folder ?: return@mapIndexedNotNull null
                    WatchProtocol.Category(
                        id = folder.node,
                        kind = kinds[folder.node]?.wire ?: WatchProtocol.KIND_SONGS,
                        name = folder.title,
                        colour = folder.colour,
                    )
                }
            }
    }
}
