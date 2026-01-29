use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use flate2::read::ZlibDecoder;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use anyhow::{anyhow, Result};
use regex::Regex;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;

const CACHE_SIZE: usize = 100;

#[derive(Debug, Clone)]
pub struct DictionaryEntry {
    pub word: String,
    pub definition: String,
}

pub struct MdxDictionary {
    file_path: String,
    header: DictionaryHeader,
    key_block_infos: Vec<KeyBlockInfo>,
    record_block_infos: Vec<RecordBlockInfo>,
    key_cache: Mutex<LruCache<String, String>>,
}

pub struct MddResource {
    file_path: String,
    header: DictionaryHeader,
    key_block_infos: Vec<KeyBlockInfo>,
    record_block_infos: Vec<RecordBlockInfo>,
    resource_cache: Mutex<LruCache<String, Vec<u8>>>,
}

#[derive(Debug)]
struct DictionaryHeader {
    version: f32,
    engine_version: String,
    format: String,
    key_case_sensitive: bool,
    strip_key: bool,
    encryption: String,
    encoding: String,
    creation_date: String,
    compact: bool,
    left2right: bool,
    data_offset: u64,
    stylesheet: HashMap<String, (String, String)>,
    title: String,
    description: String,
}

#[derive(Debug)]
struct KeyBlockInfo {
    compressed_size: u64,
    decompressed_size: u64,
    num_entries: u64,
    first_key: String,
    last_key: String,
    offset: u64,
}

#[derive(Debug)]
struct RecordBlockInfo {
    compressed_size: u64,
    decompressed_size: u64,
    offset: u64,
}

impl MdxDictionary {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file_path = path.as_ref().to_string_lossy().to_string();
        let mut file = File::open(&path)?;
        
        let header = Self::read_header(&mut file)?;
        let (key_block_infos, record_block_infos) = Self::read_block_infos(&mut file, &header)?;
        
        Ok(Self {
            file_path,
            header,
            key_block_infos,
            record_block_infos,
            key_cache: Mutex::new(LruCache::new(NonZeroUsize::new(CACHE_SIZE).unwrap())),
        })
    }

    fn read_header(file: &mut File) -> Result<DictionaryHeader> {
        // Read header length (4 bytes, big-endian)
        let header_len = file.read_u32::<BigEndian>()? as u64;
        
        // Read header data
        let mut header_data = vec![0u8; header_len as usize];
        file.read_exact(&mut header_data)?;
        
        // Parse header XML/attrs
        let header_str = String::from_utf8_lossy(&header_data);
        let header = Self::parse_header_attrs(&header_str)?;
        
        // Calculate data offset (header_len + 4 + checksum)
        let data_offset = header_len + 4 + 4; // +4 for header_len, +4 for checksum
        
        file.seek(SeekFrom::Start(data_offset))?;
        
        Ok(DictionaryHeader {
            data_offset,
            ..header
        })
    }

    fn parse_header_attrs(header_str: &str) -> Result<DictionaryHeader> {
        // Parse attributes from header string
        let mut attrs = HashMap::new();
        
        // Extract attributes using regex
        let re = Regex::new(r#"(\w+)="([^"]+)""#)?;
        for cap in re.captures_iter(header_str) {
            if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
                attrs.insert(key.as_str().to_string(), value.as_str().to_string());
            }
        }
        
        let version = attrs.get("GeneratedByEngineVersion")
            .and_then(|v| v.parse::<f32>().ok())
            .unwrap_or(2.0);
        
        Ok(DictionaryHeader {
            version,
            engine_version: attrs.get("GeneratedByEngineVersion").cloned().unwrap_or_default(),
            format: attrs.get("Format").cloned().unwrap_or_else(|| "Html".to_string()),
            key_case_sensitive: attrs.get("KeyCaseSensitive") == Some(&"Yes".to_string()),
            strip_key: attrs.get("StripKey") == Some(&"Yes".to_string()),
            encryption: attrs.get("Encryption").cloned().unwrap_or_default(),
            encoding: attrs.get("Encoding").cloned().unwrap_or_else(|| "UTF-8".to_string()),
            creation_date: attrs.get("CreationDate").cloned().unwrap_or_default(),
            compact: attrs.get("Compact") == Some(&"Yes".to_string()),
            left2right: attrs.get("Left2Right") == Some(&"Yes".to_string()),
            data_offset: 0,
            stylesheet: HashMap::new(),
            title: attrs.get("Title").cloned().unwrap_or_else(|| "Dictionary".to_string()),
            description: attrs.get("Description").cloned().unwrap_or_default(),
        })
    }

    fn read_block_infos(
        file: &mut File, 
        header: &DictionaryHeader
    ) -> Result<(Vec<KeyBlockInfo>, Vec<RecordBlockInfo>)> {
        // Read key block info section
        let num_key_blocks = file.read_u64::<BigEndian>()?;
        let num_entries = file.read_u64::<BigEndian>()?;
        
        if header.version >= 2.0 {
            // Skip key block info decompressed size (8 bytes) and 5 bytes of zeros
            let _ = file.read_u64::<BigEndian>()?;
            let mut zeros = [0u8; 5];
            file.read_exact(&mut zeros)?;
        }
        
        let key_block_info_size = file.read_u64::<BigEndian>()?;
        let _key_blocks_size = file.read_u64::<BigEndian>()?;
        
        // Read and decompress key block info
        let mut key_block_info_compressed = vec![0u8; key_block_info_size as usize];
        file.read_exact(&mut key_block_info_compressed)?;
        
        let key_block_info_data = Self::decompress(&key_block_info_compressed, 
            if header.version >= 2.0 { Some(4) } else { None })?;
        
        let key_block_infos = Self::parse_key_block_info(&key_block_info_data, num_key_blocks, header)?;
        
        // Read record block info section
        let num_record_blocks = file.read_u64::<BigEndian>()?;
        let _num_entries = file.read_u64::<BigEndian>()?;
        let record_block_info_size = file.read_u64::<BigEndian>()?;
        let _record_blocks_size = file.read_u64::<BigEndian>()?;
        
        // Read record block info (not compressed)
        let mut record_block_info_data = vec![0u8; record_block_info_size as usize];
        file.read_exact(&mut record_block_info_data)?;
        
        let record_block_infos = Self::parse_record_block_info(&record_block_info_data, num_record_blocks)?;
        
        Ok((key_block_infos, record_block_infos))
    }

    fn decompress(data: &[u8], header_size: Option<usize>) -> Result<Vec<u8>> {
        let header_size = header_size.unwrap_or(0);
        let compression_type = if header_size > 0 && !data.is_empty() {
            data[header_size - 1]
        } else if !data.is_empty() {
            data[0]
        } else {
            return Ok(vec![]);
        };
        
        let data_to_decompress = if header_size > 0 && data.len() > header_size {
            &data[header_size..]
        } else if header_size > 0 {
            return Ok(vec![]);
        } else {
            data
        };
        
        match compression_type {
            0 => {
                // No compression
                Ok(data_to_decompress.to_vec())
            }
            1 => {
                // LZO compression (not implemented, return empty)
                Ok(vec![])
            }
            2 => {
                // Zlib compression
                let mut decoder = ZlibDecoder::new(data_to_decompress);
                let mut result = Vec::new();
                decoder.read_to_end(&mut result)?;
                Ok(result)
            }
            _ => Err(anyhow!("Unknown compression type: {}", compression_type)),
        }
    }

    fn parse_key_block_info(
        data: &[u8], 
        num_blocks: u64,
        header: &DictionaryHeader
    ) -> Result<Vec<KeyBlockInfo>> {
        let mut infos = Vec::new();
        let mut cursor = std::io::Cursor::new(data);
        
        for _ in 0..num_blocks {
            let compressed_size = cursor.read_u64::<BigEndian>()?;
            let decompressed_size = cursor.read_u64::<BigEndian>()?;
            let num_entries = cursor.read_u64::<BigEndian>()?;
            
            // Read first and last key
            let first_key = Self::read_key(&mut cursor, header)?;
            let last_key = Self::read_key(&mut cursor, header)?;
            
            infos.push(KeyBlockInfo {
                compressed_size,
                decompressed_size,
                num_entries,
                first_key,
                last_key,
                offset: 0, // Will be calculated later
            });
        }
        
        // Calculate offsets
        let mut current_offset = 0u64;
        for info in &mut infos {
            info.offset = current_offset;
            current_offset += info.compressed_size;
        }
        
        Ok(infos)
    }

    fn read_key(cursor: &mut std::io::Cursor<&[u8]>, header: &DictionaryHeader) -> Result<String> {
        let len = if header.version >= 2.0 {
            cursor.read_u16::<BigEndian>()? as usize
        } else {
            cursor.read_u8()? as usize
        };
        
        let mut key_bytes = vec![0u8; len];
        cursor.read_exact(&mut key_bytes)?;
        
        // Skip the offset (8 bytes)
        cursor.seek(SeekFrom::Current(8))?;
        
        Ok(String::from_utf8_lossy(&key_bytes).to_string())
    }

    fn parse_record_block_info(data: &[u8], num_blocks: u64) -> Result<Vec<RecordBlockInfo>> {
        let mut infos = Vec::new();
        let mut cursor = std::io::Cursor::new(data);
        let mut current_offset = 0u64;
        
        for _ in 0..num_blocks {
            let compressed_size = cursor.read_u64::<BigEndian>()?;
            let decompressed_size = cursor.read_u64::<BigEndian>()?;
            
            infos.push(RecordBlockInfo {
                compressed_size,
                decompressed_size,
                offset: current_offset,
            });
            
            current_offset += compressed_size;
        }
        
        Ok(infos)
    }

    pub fn lookup(&self, word: &str) -> Option<DictionaryEntry> {
        // Check cache first
        {
            let mut cache = self.key_cache.lock().unwrap();
            if let Some(definition) = cache.get(word) {
                return Some(DictionaryEntry {
                    word: word.to_string(),
                    definition: definition.clone(),
                });
            }
        }
        
        // Find the key block containing this word
        let target_word = if self.header.strip_key {
            word.trim().to_lowercase()
        } else {
            word.to_string()
        };
        
        for (block_idx, block_info) in self.key_block_infos.iter().enumerate() {
            if target_word >= block_info.first_key && target_word <= block_info.last_key {
                if let Ok(Some((found_word, record_offset, record_size))) = 
                    self.search_in_key_block(block_idx, &target_word) {
                    if let Ok(definition) = self.read_record(record_offset, record_size) {
                        let entry = DictionaryEntry {
                            word: found_word.clone(),
                            definition: definition.clone(),
                        };
                        
                        // Cache the result
                        let mut cache = self.key_cache.lock().unwrap();
                        cache.put(found_word, definition);
                        
                        return Some(entry);
                    }
                }
            }
        }
        
        None
    }

    fn search_in_key_block(&self, block_idx: usize, target: &str) -> Result<Option<(String, u64, u64)>> {
        let block_info = &self.key_block_infos[block_idx];
        let mut file = File::open(&self.file_path)?;
        
        // Seek to key block data
        let key_data_offset = self.header.data_offset + 
            self.key_block_infos.iter().take(block_idx).map(|b| b.compressed_size).sum::<u64>();
        file.seek(SeekFrom::Start(key_data_offset))?;
        
        // Read compressed key block
        let mut compressed = vec![0u8; block_info.compressed_size as usize];
        file.read_exact(&mut compressed)?;
        
        // Decompress
        let decompressed = Self::decompress(&compressed, 
            if self.header.version >= 2.0 { Some(4) } else { None })?;
        
        // Parse entries
        let mut cursor = std::io::Cursor::new(&decompressed);
        let mut last_offset = 0u64;
        
        for _ in 0..block_info.num_entries {
            let key = Self::read_key_entry(&mut cursor, self.header.version)?;
            let offset = cursor.read_u64::<BigEndian>()?;
            
            if &key == target {
                let record_size = if offset > last_offset {
                    offset - last_offset
                } else {
                    0
                };
                return Ok(Some((key, last_offset, record_size)));
            }
            
            last_offset = offset;
        }
        
        Ok(None)
    }

    fn read_key_entry(cursor: &mut std::io::Cursor<&[u8]>, version: f32) -> Result<String> {
        let len = if version >= 2.0 {
            cursor.read_u16::<BigEndian>()? as usize
        } else {
            cursor.read_u8()? as usize
        };
        
        let mut key_bytes = vec![0u8; len];
        cursor.read_exact(&mut key_bytes)?;
        
        Ok(String::from_utf8_lossy(&key_bytes).to_string())
    }

    fn read_record(&self, offset: u64, size: u64) -> Result<String> {
        let mut file = File::open(&self.file_path)?;
        
        // Find the record block containing this offset
        let mut current_offset = 0u64;
        for block_info in &self.record_block_infos {
            if offset >= current_offset && offset < current_offset + block_info.decompressed_size {
                let block_offset = offset - current_offset;
                
                // Calculate where record blocks start
                let record_data_start = self.header.data_offset +
                    self.key_block_infos.iter().map(|b| b.compressed_size).sum::<u64>() +
                    self.record_block_infos.iter()
                        .take_while(|b| b.offset < block_info.offset)
                        .map(|b| b.compressed_size)
                        .sum::<u64>();
                
                file.seek(SeekFrom::Start(record_data_start))?;
                
                // Read and decompress record block
                let mut compressed = vec![0u8; block_info.compressed_size as usize];
                file.read_exact(&mut compressed)?;
                
                let decompressed = Self::decompress(&compressed,
                    if self.header.version >= 2.0 { Some(4) } else { None })?;
                
                // Extract record data
                let start = block_offset as usize;
                let end = (block_offset + size) as usize;
                if end <= decompressed.len() {
                    return Ok(String::from_utf8_lossy(&decompressed[start..end]).to_string());
                } else {
                    return Ok(String::from_utf8_lossy(&decompressed[start..]).to_string());
                }
            }
            current_offset += block_info.decompressed_size;
        }
        
        Err(anyhow!("Record not found at offset {}", offset))
    }

    pub fn prefix_search(&self, prefix: &str) -> Vec<String> {
        let mut results = Vec::new();
        let prefix_lower = prefix.to_lowercase();
        
        // Search in all key blocks
        for (block_idx, block_info) in self.key_block_infos.iter().enumerate() {
            if let Ok(entries) = self.read_key_block_entries(block_idx) {
                for (key, _, _) in entries {
                    if key.to_lowercase().starts_with(&prefix_lower) {
                        results.push(key);
                        if results.len() >= 20 {
                            return results;
                        }
                    }
                }
            }
        }
        
        results
    }

    fn read_key_block_entries(&self, block_idx: usize) -> Result<Vec<(String, u64, u64)>> {
        let block_info = &self.key_block_infos[block_idx];
        let mut file = File::open(&self.file_path)?;
        
        let key_data_offset = self.header.data_offset + 
            self.key_block_infos.iter().take(block_idx).map(|b| b.compressed_size).sum::<u64>();
        file.seek(SeekFrom::Start(key_data_offset))?;
        
        let mut compressed = vec![0u8; block_info.compressed_size as usize];
        file.read_exact(&mut compressed)?;
        
        let decompressed = Self::decompress(&compressed,
            if self.header.version >= 2.0 { Some(4) } else { None })?;
        
        let mut cursor = std::io::Cursor::new(&decompressed);
        let mut entries = Vec::new();
        let mut last_offset = 0u64;
        
        for _ in 0..block_info.num_entries {
            let key = Self::read_key_entry(&mut cursor, self.header.version)?;
            let offset = cursor.read_u64::<BigEndian>()?;
            let size = if offset > last_offset { offset - last_offset } else { 0 };
            
            entries.push((key, last_offset, size));
            last_offset = offset;
        }
        
        Ok(entries)
    }
}

impl MddResource {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file_path = path.as_ref().to_string_lossy().to_string();
        let mut file = File::open(&path)?;
        
        let header = Self::read_header(&mut file)?;
        let (key_block_infos, record_block_infos) = MdxDictionary::read_block_infos(&mut file, &header)?;
        
        Ok(Self {
            file_path,
            header,
            key_block_infos,
            record_block_infos,
            resource_cache: Mutex::new(LruCache::new(NonZeroUsize::new(CACHE_SIZE).unwrap())),
        })
    }

    fn read_header(file: &mut File) -> Result<DictionaryHeader> {
        MdxDictionary::read_header(file)
    }

    pub fn locate(&self, resource_name: &str) -> Option<Vec<u8>> {
        // Normalize resource name
        let name = if resource_name.starts_with('/') {
            resource_name.to_string()
        } else {
            format!("/{}", resource_name)
        };
        
        // Check cache
        {
            let mut cache = self.resource_cache.lock().unwrap();
            if let Some(data) = cache.get(&name) {
                return Some(data.clone());
            }
        }
        
        // Search for resource
        for (block_idx, block_info) in self.key_block_infos.iter().enumerate() {
            if let Ok(Some((_, record_offset, record_size))) = self.search_in_key_block(block_idx, &name) {
                if let Ok(data) = self.read_record(record_offset, record_size) {
                    let mut cache = self.resource_cache.lock().unwrap();
                    cache.put(name, data.clone());
                    return Some(data);
                }
            }
        }
        
        None
    }
}

impl MddResource {
    fn search_in_key_block(&self, block_idx: usize, target: &str) -> Result<Option<(String, u64, u64)>> {
        let block_info = &self.key_block_infos[block_idx];
        let mut file = File::open(&self.file_path)?;
        
        // Calculate offset to key block data
        let header_len = self.header.data_offset;
        let key_blocks_start = header_len;
        let block_offset: u64 = self.key_block_infos.iter().take(block_idx).map(|b| b.compressed_size).sum();
        
        file.seek(SeekFrom::Start(key_blocks_start + block_offset))?;
        
        let mut compressed = vec![0u8; block_info.compressed_size as usize];
        file.read_exact(&mut compressed)?;
        
        let decompressed = Self::decompress(&compressed,
            if self.header.version >= 2.0 { Some(4) } else { None })?;
        
        let mut cursor = std::io::Cursor::new(&decompressed);
        let mut last_offset = 0u64;
        
        for _ in 0..block_info.num_entries {
            let key = Self::read_key_entry(&mut cursor, self.header.version)?;
            let offset = cursor.read_u64::<BigEndian>()?;
            let size = if offset > last_offset { offset - last_offset } else { 0 };
            
            if &key == target {
                return Ok(Some((key, last_offset, size)));
            }
            
            last_offset = offset;
        }
        
        Ok(None)
    }

    fn read_record(&self, offset: u64, size: u64) -> Result<Vec<u8>> {
        let mut file = File::open(&self.file_path)?;
        
        // Calculate record blocks start position
        let record_blocks_start = self.header.data_offset + 
            self.key_block_infos.iter().map(|b| b.compressed_size).sum::<u64>();
        
        // Find the record block containing this offset
        let mut current_offset = 0u64;
        let mut block_file_offset = 0u64;
        
        for block_info in &self.record_block_infos {
            if offset >= current_offset && offset < current_offset + block_info.decompressed_size {
                let block_offset = offset - current_offset;
                
                file.seek(SeekFrom::Start(record_blocks_start + block_file_offset))?;
                
                let mut compressed = vec![0u8; block_info.compressed_size as usize];
                file.read_exact(&mut compressed)?;
                
                let decompressed = Self::decompress(&compressed,
                    if self.header.version >= 2.0 { Some(4) } else { None })?;
                
                let start = block_offset as usize;
                let end = ((block_offset + size) as usize).min(decompressed.len());
                
                return Ok(decompressed[start..end].to_vec());
            }
            current_offset += block_info.decompressed_size;
            block_file_offset += block_info.compressed_size;
        }
        
        Err(anyhow!("Record not found at offset {}", offset))
    }

    fn read_key_entry(cursor: &mut std::io::Cursor<&[u8]>, version: f32) -> Result<String> {
        let len = if version >= 2.0 {
            cursor.read_u16::<BigEndian>()? as usize
        } else {
            cursor.read_u8()? as usize
        };
        
        let mut key_bytes = vec![0u8; len];
        cursor.read_exact(&mut key_bytes)?;
        
        Ok(String::from_utf8_lossy(&key_bytes).to_string())
    }

    fn decompress(data: &[u8], header_size: Option<usize>) -> Result<Vec<u8>> {
        let header_size = header_size.unwrap_or(0);
        let compression_type = if header_size > 0 && data.len() > header_size {
            data[header_size - 1]
        } else if !data.is_empty() {
            data[0]
        } else {
            return Ok(vec![]);
        };
        
        let data_to_decompress = if header_size > 0 && data.len() > header_size {
            &data[header_size..]
        } else {
            data
        };
        
        match compression_type {
            0 => Ok(data_to_decompress.to_vec()),
            1 => Ok(vec![]), // LZO not implemented
            2 => {
                let mut decoder = ZlibDecoder::new(data_to_decompress);
                let mut result = Vec::new();
                decoder.read_to_end(&mut result)?;
                Ok(result)
            }
            _ => Err(anyhow!("Unknown compression type: {}", compression_type)),
        }
    }
}
