/*
===========================================================================
Copyright (C) 1999-2005 Id Software, Inc.

This file is part of Quake III Arena source code.

Quake III Arena source code is free software; you can redistribute it
and/or modify it under the terms of the GNU General Public License as
published by the Free Software Foundation; either version 2 of the License,
or (at your option) any later version.

Quake III Arena source code is distributed in the hope that it will be
useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with Quake III Arena source code; if not, write to the Free Software
Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA  02110-1301  USA
===========================================================================
*/
// qcommon.h -- definitions common between client and server, but not game.or ref modules
#ifndef _QCOMMON_H_
#define _QCOMMON_H_

#include <sys/types.h>
#include "cm_public.h"

//Ignore __attribute__ on non-gcc/clang platforms
#if !defined(__GNUC__) && !defined(__clang__)
#ifndef __attribute__
#define __attribute__(x)
#endif
#endif

/* C99 defines __func__ */
#if __STDC_VERSION__ < 199901L 
#if __GNUC__ >= 2 || _MSC_VER >= 1300 
#define __func__ __FUNCTION__ 
#else 
#define __func__ "(unknown)" 
#endif
#endif

#if defined (_WIN32) || defined(__linux__)
#define USE_AFFINITY_MASK
#endif

// stringify macro
#define XSTRING(x)	STRING(x)
#define STRING(x)	#x

//#define	PRE_RELEASE_DEMO
#define DELAY_WRITECONFIG

/*
==============================================================

VIRTUAL MACHINE

==============================================================
*/

typedef enum {
	TRAP_MEMSET = 100,
	TRAP_MEMCPY,
	TRAP_STRNCPY,
	TRAP_SIN,
	TRAP_COS,
	TRAP_ATAN2,
	TRAP_SQRT,
} sharedTraps_t;

/*
==============================================================

FILESYSTEM

No stdio calls should be used by any part of the game, because
we need to deal with all sorts of directory and separator char
issues.
==============================================================
*/

// referenced flags
// these are in loop specific order so don't change the order
#define FS_GENERAL_REF	0x01
#define FS_UI_REF		0x02
#define FS_CGAME_REF	0x04
// number of id paks that will never be autodownloaded from baseq3/missionpack
#define NUM_ID_PAKS		9
#define NUM_TA_PAKS		4

typedef enum {
	H_SYSTEM,
	H_QAGAME,
	H_CGAME,
	H_Q3UI
} handleOwner_t;

#define FS_MATCH_EXTERN    (1<<0)
#define FS_MATCH_PURE      (1<<1)
#define FS_MATCH_UNPURE    (1<<2)
#define FS_MATCH_STICK     (1<<3)
#define FS_MATCH_SUBDIRS   (1<<4)
#define FS_MATCH_PK3s      (FS_MATCH_PURE | FS_MATCH_UNPURE)
#define FS_MATCH_ANY       (FS_MATCH_EXTERN | FS_MATCH_PURE | FS_MATCH_UNPURE)

#define FS_MAX_SUBDIRS		8 /* should be enough for practical use with FS_MATCH_SUBDIRS */

#define	MAX_FILE_HANDLES	64
#define	FS_INVALID_HANDLE	0

#define	MAX_FOUND_FILES		0x5000

#ifdef DEDICATED
#define Q3CONFIG_CFG "q3config_server.cfg"
#define CONSOLE_HISTORY_FILE "q3history_server"
#else
#define Q3CONFIG_CFG "q3config.cfg"
#define CONSOLE_HISTORY_FILE "q3history"
#endif

typedef	time_t fileTime_t;
#if defined  (_MSC_VER) && defined (__clang__)
typedef	_off_t  fileOffset_t;
#else
typedef	off_t  fileOffset_t;
#endif

qboolean FS_Initialized( void );

void	FS_InitFilesystem ( void );
void	FS_Shutdown( qboolean closemfp );

qboolean	FS_ConditionalRestart( int checksumFeed, qboolean clientRestart );

void	FS_Restart( int checksumFeed );
// shutdown and restart the filesystem so changes to fs_gamedir can take effect

void	FS_Reload( void );

char	**FS_ListFiles( const char *directory, const char *extension, int *numfiles );
// directory should not have either a leading or trailing /
// if extension is "/", only subdirectories will be returned
// the returned files will not include any directories or /

void	FS_FreeFileList( char **list );

qboolean FS_FileExists( const char *file );

char   *FS_BuildOSPath( const char *base, const char *game, const char *qpath );

qboolean FS_CompareZipChecksum( const char *zipfile );
int		FS_GetZipChecksum( const char *zipfile );

int		FS_LoadStack( void );

int		FS_GetFileList(  const char *path, const char *extension, char *listbuf, int bufsize );

fileHandle_t	FS_FOpenFileWrite( const char *qpath );
fileHandle_t	FS_FOpenFileAppend( const char *filename );
// will properly create any needed paths and deal with separator character issues

qboolean FS_ResetReadOnlyAttribute( const char *filename );

qboolean FS_SV_FileExists( const char *file );

fileHandle_t FS_SV_FOpenFileWrite( const char *filename );
int		FS_SV_FOpenFileRead( const char *filename, fileHandle_t *fp );
void	FS_SV_Rename( const char *from, const char *to );
int		FS_FOpenFileRead( const char *qpath, fileHandle_t *file, qboolean uniqueFILE );
// if uniqueFILE is true, then a new FILE will be fopened even if the file
// is found in an already open pak file.  If uniqueFILE is false, you must call
// FS_FCloseFile instead of fclose, otherwise the pak FILE would be improperly closed
// It is generally safe to always set uniqueFILE to true, because the majority of
// file IO goes through FS_ReadFile, which Does The Right Thing already.

void FS_TouchFileInPak( const char *filename );

void FS_BypassPure( void );
void FS_RestorePure( void );

int FS_Home_FOpenFileRead( const char *filename, fileHandle_t *file );

qboolean FS_FileIsInPAK( const char *filename, int *pChecksum, char *pakName );
// returns qtrue if a file is in the PAK file, otherwise qfalse

int		FS_PakIndexForHandle( fileHandle_t f );

// returns pak index or -1 if file is not in pak
extern int fs_lastPakIndex;

extern qboolean fs_reordered;

int		FS_Write( const void *buffer, int len, fileHandle_t f );

int		FS_Read( void *buffer, int len, fileHandle_t f );
// properly handles partial reads and reads from other dlls

void	FS_FCloseFile( fileHandle_t f );
// note: you can't just fclose from another DLL, due to MS libc issues

int		FS_ReadFile( const char *qpath, void **buffer );
// returns the length of the file
// a null buffer will just return the file length without loading
// as a quick check for existence. -1 length == not present
// A 0 byte will always be appended at the end, so string ops are safe.
// the buffer should be considered read-only, because it may be cached
// for other uses.

void	FS_ForceFlush( fileHandle_t f );
// forces flush on files we're writing to.

void	FS_FreeFile( void *buffer );
// frees the memory returned by FS_ReadFile

void	FS_WriteFile( const char *qpath, const void *buffer, int size );
// writes a complete file, creating any subdirectories needed

int		FS_filelength( fileHandle_t f );
// doesn't work for files that are opened from a pack file

int		FS_FTell( fileHandle_t f );
// where are we?

void	FS_Flush( fileHandle_t f );

void 	QDECL FS_Printf( fileHandle_t f, const char *fmt, ... ) __attribute__ ((format (printf, 2, 3)));
// like fprintf

int		FS_FOpenFileByMode( const char *qpath, fileHandle_t *f, fsMode_t mode );
// opens a file for reading, writing, or appending depending on the value of mode

int		FS_Seek( fileHandle_t f, long offset, fsOrigin_t origin );
// seek on a file

qboolean FS_FilenameCompare( const char *s1, const char *s2 );

const char *FS_LoadedPakNames( void );
const char *FS_LoadedPakChecksums( qboolean *overflowed );
// Returns a space separated string containing the checksums of all loaded pk3 files.
// Servers with sv_pure set will get this string and pass it to clients.

qboolean FS_ExcludeReference( void );
const char *FS_ReferencedPakNames( void );
const char *FS_ReferencedPakChecksums( void );
const char *FS_ReferencedPakPureChecksums( int maxlen );
// Returns a space separated string containing the checksums of all loaded 
// AND referenced pk3 files. Servers with sv_pure set will get this string 
// back from clients for pure validation 

void FS_ClearPakReferences( int flags );
// clears referenced booleans on loaded pk3s

void FS_PureServerSetReferencedPaks( const char *pakSums, const char *pakNames );
void FS_PureServerSetLoadedPaks( const char *pakSums, const char *pakNames );
// If the string is empty, all data sources will be allowed.
// If not empty, only pk3 files that match one of the space
// separated checksums will be checked for files, with the
// sole exception of .cfg files.

qboolean FS_IsPureChecksum( int sum );

qboolean FS_InvalidGameDir( const char *gamedir );
qboolean FS_idPak( const char *pak, const char *base, int numPaks );
qboolean FS_ComparePaks( char *neededpaks, int len, qboolean dlstring );

void FS_Rename( const char *from, const char *to );

void FS_Remove( const char *osPath );
void FS_HomeRemove( const char *homePath );

void	FS_FilenameCompletion( const char *dir, const char *ext, qboolean stripExt, void(*callback)(const char *s), int flags );

int FS_VM_OpenFile( const char *qpath, fileHandle_t *f, fsMode_t mode, handleOwner_t owner );
int FS_VM_ReadFile( void *buffer, int len, fileHandle_t f, handleOwner_t owner );
void FS_VM_WriteFile( void *buffer, int len, fileHandle_t f, handleOwner_t owner );
int FS_VM_SeekFile( fileHandle_t f, long offset, fsOrigin_t origin, handleOwner_t owner );
void FS_VM_CloseFile( fileHandle_t f, handleOwner_t owner );
void FS_VM_CloseFiles( handleOwner_t owner );

const char *FS_GetCurrentGameDir( void );
const char *FS_GetBaseGameDir( void );

const char *FS_GetHomePath( void );

qboolean FS_StripExt( char *filename, const char *ext );
qboolean FS_AllowedExtension( const char *fileName, qboolean allowPk3s, const char **ext );

void *FS_LoadLibrary( const char *name );

typedef qboolean ( *fnamecallback_f )( const char *filename, int length );

void FS_SetFilenameCallback( fnamecallback_f func ); 

char *FS_CopyString( const char *in );

/*
==============================================================

MISC

==============================================================
*/

// TTimo
// centralized and cleaned, that's the max string you can send to a Com_Printf / Com_DPrintf (above gets truncated)
// bump to 8192 as 4096 may be not enough to print some data like gl extensions - CE
#define	MAXPRINTMSG	8192

char		*CopyString( const char *in );

void 		QDECL Com_Printf( const char *fmt, ... ) __attribute__ ((format (printf, 1, 2)));
void 		QDECL Com_DPrintf( const char *fmt, ... ) __attribute__ ((format (printf, 1, 2)));

static ID_INLINE unsigned int log2pad( unsigned int v, int roundup )
{
	unsigned int x = 1;

	while ( x < v ) x <<= 1;

	if ( roundup == 0 ) {
		if ( x > v ) {
			x >>= 1;
		}
	}

	return x;
}

extern	qboolean	com_errorEntered;

typedef enum {
	TAG_FREE,
	TAG_GENERAL,
	TAG_PACK,
	TAG_SEARCH_PATH,
	TAG_SEARCH_PACK,
	TAG_SEARCH_DIR,
	TAG_BOTLIB,
	TAG_RENDERER,
	TAG_CLIENTS,
	TAG_SMALL,
	TAG_STATIC,
	TAG_COUNT
} memtag_t;

/*

--- low memory ----
server vm
server clipmap
---mark---
renderer initialization (shaders, etc)
UI vm
cgame vm
renderer map
renderer models

---free---

temp file loading
--- high memory ---

*/

#if defined(_DEBUG) && !defined(BSPC)
	#define ZONE_DEBUG
#endif

#ifdef ZONE_DEBUG
#define Z_TagMalloc(size, tag)			Z_TagMallocDebug(size, tag, #size, __FILE__, __LINE__)
#define Z_Malloc(size)					Z_MallocDebug(size, #size, __FILE__, __LINE__)
#define S_Malloc(size)					S_MallocDebug(size, #size, __FILE__, __LINE__)
void *Z_TagMallocDebug( int size, memtag_t tag, char *label, char *file, int line );	// NOT 0 filled memory
void *Z_MallocDebug( int size, char *label, char *file, int line );			// returns 0 filled memory
void *S_MallocDebug( int size, char *label, char *file, int line );			// returns 0 filled memory
#else
void *Z_TagMalloc( int size, memtag_t tag );	// NOT 0 filled memory
void *Z_Malloc( int size );			// returns 0 filled memory
void *S_Malloc( int size );			// NOT 0 filled memory only for small allocations
#endif
void Z_Free( void *ptr );
int Z_FreeTags( memtag_t tag );
int Z_AvailableMemory( void );
void Z_LogHeap( void );

void Hunk_Clear( void );
void Hunk_ClearToMark( void );
void Hunk_SetMark( void );
qboolean Hunk_CheckMark( void );
void Hunk_ClearTempMemory( void );
void *Hunk_AllocateTempMemory( int size );
void Hunk_FreeTempMemory( void *buf );
int	Hunk_MemoryRemaining( void );
void Hunk_Log( void);

void Com_Init( void );

#endif // _QCOMMON_H_
