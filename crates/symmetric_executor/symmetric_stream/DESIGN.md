# Symmetric stream detailed design

## Elements
synchronization
- `read_ready_event_send` R: activate, W: subscribe, one call might indicate multiple events
- `write_ready_event_send` R: subscribe, W: activate

reader to writer communication
- `read_addr` R: replace with new (second), W: replace with null (second)
- `read_size` synchronization point R: check zero (first), replace zero with size (third), W: check non-zero (first), replace with zero (third)
- `read_closed` R: set once, W: information

writer to reader communication
- `ready_addr` R: 
- `ready_size` synchronization point R: check non-zero (first), replace with zero (third), W: replace with size (second)
- `ready_capacity`
- `write_closed` R: information, W: set once

## Interaction sequence
This sequence is compatible with many concurrent readers and writers, 
for a one sided channel setting to -1 isn't necessary to ensure 
a consistent set of size+addr+capacity communicated. 
This is because in the single case the size won't change after testing.

### writing side
1. acquire or reset signal
2. compare size to zero and (optionally) replace with -1 (busy)
3. **fail**: check for closed
   - **non closed**: *wait* for signal, then back to #1
   - **closed**: *stop* writing, return
4. **ok**: slot is locked for this writer, write addr (and capacity), then size (relase), signal reading side

**any time**: Set closed, signal reader

### reading side
1. acquire or reset signal
2. compare size to non-(zero or -1), this step avoids temporarily setting slots to -1 by readers when there is no information available
3. **zero or -1**: check for closed
   - **closed**: *stop* reading, return
   - **not closed**: *wait* for signal, then back to #1
4. **valid**: slot is ready, (optional, otherwise proceed with ok) compare with previous value and replace with -1 (busy)
   - **fail**: *wait* for signal, then back to #1
   - **ok**: Slot acquired, read addr (and capacity), replace size with zero (release), <br>
     (??? signal writer ??? likely done by the other direction channel)

**any time**: Set closed, signal writer

## Writing sequence
- `write-ready-subscribe`

on event
- `start-writing`
- `finish-writing`

### Signalling
- `read-ready-activate`

### Out of band information
- `is-read-closed`

## Reading sequence
- `start-reading`
- `read-ready-subscribe`

on event
- `read-result`

### Signalling
- `write-ready-activate`
- `close-read`

### Out of band information
- `is-write-closed`

## Potential extensions
- extend towards queueing multiple buffers in flight
