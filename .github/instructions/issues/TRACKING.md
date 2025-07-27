# Issues Tracking

## Active Issues

*No active issues - GS-31 completed successfully*

---

## Completed Tasks (Recent)

### GS-31: CLI Filtering Flags Implementation
**Status**: ✅ COMPLETED  
**Completed**: 2025-07-27  
**Priority**: High  
**Type**: Enhancement  

**Summary**: Add comprehensive filtering flags to CLI for date ranges, file paths, and authors

**Completed Features**:
- ✅ Date filtering with ISO 8601 and relative formats (--since, --until)
- ✅ Path filtering with include/exclude patterns (--include-path, --exclude-path) 
- ✅ File filtering with glob patterns (--include-file, --exclude-file)
- ✅ Author filtering with include/exclude (--author, --exclude-author)
- ✅ Result control with limits and plugins (--limit, plugins)
- ✅ **BONUS**: Scanner configuration (--performance-mode, --max-memory, --queue-size)
- ✅ Memory size parsing with units (512MB, 1GB, 0.5T, etc.)
- ✅ Comprehensive validation and error handling
- ✅ Integration with GS-24 filtering system

**Test Results**: 196 total tests passing (131 library + 65 CLI)

**Location**: `/docs/issues/GS-31-CLI-Filtering-Flags.md`

### GS-24: Core Scanner Infrastructure (Steps 1-8)
**Status**: ✅ COMPLETED (Steps 1-8), Step 9 Pending  
**Completed**: 2025-07-27  

**Summary**: Foundational scanner module with API versioning, filtering system, and zero-cost abstractions

**Completed Steps**:
- ✅ Step 1: Module Infrastructure 
- ✅ Step 2: API Versioning System (with Cargo.toml fix)
- ✅ Step 3: Scanning Modes & Bitflags
- ✅ Step 4: Message Structures  
- ✅ Step 5: Configuration System
- ✅ Step 6: Core Traits Definition
- ✅ Step 7: Query Parameter System (13 tests)
- ✅ Step 8: Filtering System (13 tests) - Zero-cost abstractions with ControlFlow

**Test Coverage**: 76 total tests passing (including 13 filtering system tests)

**Pending**: Step 9 - Integration and API Finalization

---

## Issue Numbering Convention

Based on existing patterns:
- **GS-1 to GS-14**: Core infrastructure (completed)
- **GS-15**: Repository Scanner MVP (split into GS-24 through GS-30)
- **GS-24**: Core Scanner Infrastructure (active/completed)
- **GS-25 to GS-30**: Additional scanner components (pending)
- **GS-31+**: New enhancements and features

## Next Priority Items

1. **GS-24 Step 9**: Complete scanner integration and API finalization (MEDIUM - Can proceed with GS-31 complete)
2. **GS-25**: Git Integration (next scanner component)
3. **GS-26-30**: Remaining scanner components
4. **New Features**: Ready to define next enhancement requirements
