# Cache Management

## Overview

The launcher caches downloaded artifacts locally to improve performance and enable offline execution.

## Storage Structure

```
cache/
    tool-name/
        version/
            executable files
    cache_index.json
```

## Configuration

- Default cache limit: **500 MB**
- Configurable via `cache.max_size_mb` in launcher configuration

## Cache Behavior

- Automatic retention of latest versions
- Configurable cache size limit
- Automatic LRU (Least Recently Used) eviction when limits are reached
- Index-based tracking of cached entries with timestamps

## API

### CacheManager
- `new(cache_dir, max_size_mb)` - Create manager with directory and size limit
- `init()` - Initialize cache directory and index
- `get_cached_path(tool, version)` - Look up cached artifact path
- `store(tool, version, source_path)` - Store artifact in cache
- `touch(tool, version)` - Update last-used timestamp
- `remove(tool, version)` - Manually remove cached artifact
- `total_size()` - Get total cache size in bytes
- `list_entries()` - List all cached entries

## Eviction Strategy

When the cache exceeds the configured size limit, entries are evicted in LRU order (least recently used first) until the total size is within limits.
