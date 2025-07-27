# Issues Tracking

## Active Issues

### GS-31: CLI Filtering Flags Implementation
**Status**: Open  
**Priority**: High  
**Type**: Enhancement  
**Created**: 2025-07-27  

**Summary**: Add comprehensive filtering flags to CLI for date ranges, file paths, and authors

**Dependencies**: 
- ✅ GS-24 (Filtering System) - COMPLETED

**Description**: 
The current CLI only supports basic repository and logging flags. Need to add filtering capabilities that integrate with the completed filtering system from GS-24.

**Location**: `/docs/issues/GS-31-CLI-Filtering-Flags.md`

---

## Completed Tasks (Recent)

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

1. **GS-31**: CLI Filtering Flags (HIGH - Needed for testing and usability)
2. **GS-24 Step 9**: Complete scanner integration and API finalization  
3. **GS-25**: Git Integration (next scanner component)
4. **GS-26-30**: Remaining scanner components
