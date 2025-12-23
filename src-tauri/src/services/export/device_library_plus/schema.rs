//! Database schema for Device Library Plus format.
//!
//! Contains CREATE TABLE statements for all 22 tables in the Device Library Plus database.
//! Schema is based on pyrekordbox/devicelib_plus/models.py

use rusqlite::Connection;

use crate::error::Result;

/// Creates all tables in the Device Library Plus database
pub fn create_all_tables(conn: &Connection) -> Result<()> {
    // Order matters due to foreign key constraints
    conn.execute_batch(
        r#"
        -- Enable foreign keys
        PRAGMA foreign_keys = ON;

        -- 1. Artist table
        CREATE TABLE IF NOT EXISTS artist (
            artist_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) UNIQUE NOT NULL,
            nameForSearch VARCHAR(255)
        );

        -- 2. Image table (must be before album, content, playlist, hotCueBankList)
        CREATE TABLE IF NOT EXISTS image (
            image_id INTEGER PRIMARY KEY AUTOINCREMENT,
            path VARCHAR(255) UNIQUE NOT NULL
        );

        -- 3. Album table
        CREATE TABLE IF NOT EXISTS album (
            album_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) UNIQUE NOT NULL,
            artist_id INTEGER REFERENCES artist(artist_id),
            image_id INTEGER REFERENCES image(image_id),
            isComplation INTEGER,
            nameForSearch VARCHAR(255)
        );

        -- 4. Genre table
        CREATE TABLE IF NOT EXISTS genre (
            genre_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) UNIQUE NOT NULL
        );

        -- 5. Label table
        CREATE TABLE IF NOT EXISTS label (
            label_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) UNIQUE NOT NULL
        );

        -- 6. Key table (musical key)
        CREATE TABLE IF NOT EXISTS key (
            key_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) UNIQUE NOT NULL
        );

        -- 7. Color table
        CREATE TABLE IF NOT EXISTS color (
            color_id INTEGER PRIMARY KEY AUTOINCREMENT,
            name VARCHAR(255) UNIQUE NOT NULL
        );

        -- 8. Content table (main track table)
        CREATE TABLE IF NOT EXISTS content (
            content_id INTEGER PRIMARY KEY AUTOINCREMENT,
            title VARCHAR(255),
            titleForSearch VARCHAR(255),
            subtitle VARCHAR(255),
            bpmx100 INTEGER,
            length INTEGER,
            trackNo INTEGER,
            discNo INTEGER,
            artist_id_artist INTEGER REFERENCES artist(artist_id),
            artist_id_remixer INTEGER REFERENCES artist(artist_id),
            artist_id_originalArtist INTEGER REFERENCES artist(artist_id),
            artist_id_composer INTEGER REFERENCES artist(artist_id),
            artist_id_lyricist INTEGER REFERENCES artist(artist_id),
            album_id INTEGER REFERENCES album(album_id),
            genre_id INTEGER REFERENCES genre(genre_id),
            label_id INTEGER REFERENCES label(label_id),
            key_id INTEGER REFERENCES key(key_id),
            color_id INTEGER REFERENCES color(color_id),
            image_id INTEGER REFERENCES image(image_id),
            djComment VARCHAR(255),
            rating INTEGER,
            releaseYear INTEGER,
            releaseDate TEXT,
            dateCreated TEXT,
            dateAdded TEXT,
            path VARCHAR(255) UNIQUE NOT NULL,
            fileName VARCHAR(255) NOT NULL,
            fileSize INTEGER NOT NULL,
            fileType INTEGER NOT NULL,
            bitrate INTEGER NOT NULL,
            bitDepth INTEGER NOT NULL,
            samplingRate INTEGER NOT NULL,
            isrc VARCHAR(12),
            isHotCueAutoLoadOn INTEGER,
            isKuvoDeliverStatusOn INTEGER,
            kuvoDeliveryComment VARCHAR(255),
            masterDbId INTEGER,
            masterContentId INTEGER,
            analysisDataFilePath VARCHAR(255),
            analysedBits INTEGER,
            contentLink INTEGER,
            hasModified INTEGER,
            cueUpdateCount INTEGER,
            analysisDataUpdateCount INTEGER,
            informationUpdateCount INTEGER
        );

        -- 9. Cue table
        CREATE TABLE IF NOT EXISTS cue (
            cue_id INTEGER PRIMARY KEY AUTOINCREMENT,
            content_id INTEGER REFERENCES content(content_id),
            kind INTEGER,
            colorTableIndex INTEGER,
            cueComment VARCHAR(255),
            isActiveLoop INTEGER,
            beatLoopNumerator INTEGER,
            beatLoopDenominator INTEGER,
            inUsec INTEGER,
            outUsec INTEGER,
            in150FramePerSec INTEGER,
            out150FramePerSec INTEGER,
            inMpegFrameNumber INTEGER,
            outMpegFrameNumber INTEGER,
            inMpegAbs INTEGER,
            outMpegAbs INTEGER,
            inDecodingStartFramePosition INTEGER,
            outDecodingStartFramePosition INTEGER,
            inFileOffsetInBlock INTEGER,
            outFileOffsetInBlock INTEGER,
            inNumberOfSampleInBlock INTEGER,
            outNumberOfSampleInBlock INTEGER
        );

        -- 10. Playlist table
        CREATE TABLE IF NOT EXISTS playlist (
            playlist_id INTEGER PRIMARY KEY AUTOINCREMENT,
            sequenceNo INTEGER NOT NULL,
            name VARCHAR(255) NOT NULL,
            image_id INTEGER REFERENCES image(image_id),
            attribute INTEGER,
            playlist_id_parent INTEGER REFERENCES playlist(playlist_id)
        );

        -- 11. PlaylistContent table
        CREATE TABLE IF NOT EXISTS playlist_content (
            playlist_id INTEGER NOT NULL REFERENCES playlist(playlist_id),
            content_id INTEGER NOT NULL REFERENCES content(content_id),
            sequenceNo INTEGER NOT NULL,
            PRIMARY KEY (playlist_id, content_id)
        );

        -- 12. HotCueBankList table
        CREATE TABLE IF NOT EXISTS hotCueBankList (
            hotCueBankList_id INTEGER PRIMARY KEY AUTOINCREMENT,
            sequenceNo INTEGER,
            name VARCHAR(255),
            image_id INTEGER REFERENCES image(image_id),
            attribute INTEGER,
            hotCueBankList_id_parent INTEGER REFERENCES hotCueBankList(hotCueBankList_id)
        );

        -- 13. HotCueBankListCue table
        CREATE TABLE IF NOT EXISTS hotCueBankList_cue (
            hotCueBankList_id INTEGER NOT NULL REFERENCES hotCueBankList(hotCueBankList_id),
            cue_id INTEGER NOT NULL REFERENCES cue(cue_id),
            sequenceNo INTEGER,
            PRIMARY KEY (hotCueBankList_id, cue_id)
        );

        -- 14. History table
        CREATE TABLE IF NOT EXISTS history (
            history_id INTEGER PRIMARY KEY AUTOINCREMENT,
            sequenceNo INTEGER,
            name VARCHAR(255),
            attribute TEXT,
            history_id_parent INTEGER REFERENCES history(history_id)
        );

        -- 15. HistoryContent table
        CREATE TABLE IF NOT EXISTS history_content (
            history_id INTEGER NOT NULL REFERENCES history(history_id),
            content_id INTEGER NOT NULL REFERENCES content(content_id),
            sequenceNo INTEGER,
            PRIMARY KEY (history_id, content_id)
        );

        -- 16. MyTag table
        CREATE TABLE IF NOT EXISTS myTag (
            myTag_id INTEGER PRIMARY KEY AUTOINCREMENT,
            sequenceNo INTEGER NOT NULL,
            name VARCHAR(255) UNIQUE NOT NULL,
            attribute INTEGER,
            myTag_id_parent INTEGER REFERENCES myTag(myTag_id)
        );

        -- 17. MyTagContent table
        CREATE TABLE IF NOT EXISTS myTag_content (
            myTag_id INTEGER NOT NULL REFERENCES myTag(myTag_id),
            content_id INTEGER NOT NULL REFERENCES content(content_id),
            PRIMARY KEY (myTag_id, content_id)
        );

        -- 18. Property table (database metadata)
        CREATE TABLE IF NOT EXISTS property (
            deviceName VARCHAR(255) PRIMARY KEY NOT NULL,
            dbVersion INTEGER,
            numberOfContents INTEGER,
            createdDate TEXT,
            backGroundColorType INTEGER,
            myTagMasterDBID INTEGER
        );

        -- 19. RecommendedLike table
        CREATE TABLE IF NOT EXISTS recommendedLike (
            content_id_1 INTEGER NOT NULL REFERENCES content(content_id),
            content_id_2 INTEGER NOT NULL REFERENCES content(content_id),
            rating INTEGER,
            createdDate TEXT,
            PRIMARY KEY (content_id_1, content_id_2)
        );

        -- 20. MenuItem table
        CREATE TABLE IF NOT EXISTS menuItem (
            menuItem_id INTEGER PRIMARY KEY AUTOINCREMENT,
            kind INTEGER,
            name VARCHAR(255)
        );

        -- 21. Category table
        CREATE TABLE IF NOT EXISTS category (
            category_id INTEGER PRIMARY KEY AUTOINCREMENT,
            menuItem_id INTEGER REFERENCES menuItem(menuItem_id),
            sequenceNo INTEGER,
            isVisible INTEGER
        );

        -- 22. Sort table
        CREATE TABLE IF NOT EXISTS sort (
            sort_id INTEGER PRIMARY KEY AUTOINCREMENT,
            menuItem_id INTEGER NOT NULL REFERENCES menuItem(menuItem_id),
            sequenceNo INTEGER,
            isVisible INTEGER,
            isSelectedAsSubColumn INTEGER
        );
        "#,
    )?;

    Ok(())
}
