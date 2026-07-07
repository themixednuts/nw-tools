//go:build !windows

package gameassets

func decompressOodle(entryName string, _ []byte, _ uint64) ([]byte, error) {
	return nil, UnsupportedPakCompressionError{EntryName: entryName, Method: PakCompressionOodle}
}
