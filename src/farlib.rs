use alloc::{format, vec};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Struct containing information about a file, without reading the actual data of the file.
/// This should be used in cases where file information is needed to be retrieved quickly
/// (e.g. when listing files in an archive).
pub struct FarFileInfo {
    pub name: String,
    pub size: u32,
    pub offset: u32,
}

/// Struct containing a file, whether or not it's in an archive.
/// This should be used when creating a file from a buffer, or when getting files from an archive.
///
/// Should be created by calling `FarFile::new_from_archive` if extracting from an archive, or
/// `FarFile::new_from_file` if reading from a buffer.
pub struct FarFile {
    pub name: String,
    pub size: u32,
    pub data: Vec<u8>,
}

/// Struct containing information about an archive.
///
/// Should be created by one of two ways:
/// 1. Calling `FarArchive::new_from_files` if creating an archive from a list of FarFile structs
/// 2. Calling `farlib::test(buffer)` if loading an archive from a file/buffer
pub struct FarArchive {
    pub version: u32,
    pub file_count: u32,
    pub file_list: Vec<FarFileInfo>,
    pub file_data: Vec<FarFile>,
}

impl FarFile {
    /// Creates a new FarFile struct from an offset, size, and archive buffer.
    pub fn new_from_archive(name : String, size : u32, offset : u32, original_file : &Vec<u8>) -> FarFile {
        let mut data = original_file[offset as usize..(offset + size) as usize].to_vec();
        FarFile {
            name,
            size,
            data,
        }
    }

    /// Creates a new FarFile struct from a size, and data buffer.
    pub fn new_from_file(name : String, size : u32, data : Vec<u8>) -> FarFile {
        FarFile {
            name,
            size,
            data,
        }
    }
}

impl FarArchive {
    /// Creates a new FarArchive struct from a list of FarFile structs.
    /// Important when creating a new archive.
    pub fn new_from_files(files : Vec<FarFile>) -> FarArchive {
        let mut file_list = Vec::new();
        let mut file_data = Vec::new();
        let mut offset = 0;
        for file in files {
            offset += &file.size;
            file_list.push(FarFileInfo {
                name: file.name.clone(),
                size: file.size,
                offset,
            });
            file_data.push(file);
        }
        FarArchive {
            version: 1,
            file_count: file_list.len() as u32,
            file_list,
            file_data,
        }
    }

    /// Loads file data into a FarArchive struct, used if a FarFileInfo struct is not sufficient.
    pub fn load_file_data(self, original_file : &Vec<u8>) -> FarArchive {
        let mut new_file_data = Vec::new();
        for i in 0..self.file_list.len() {
            new_file_data.push(FarFile::new_from_archive(
                self.file_list[i].name.clone(),
                self.file_list[i].size,
                self.file_list[i].offset,
                original_file,
            ));
        }
        FarArchive {
            version: self.version,
            file_count: self.file_count,
            file_list: self.file_list,
            file_data: new_file_data,
        }
    }

    /// Creates a buffer representing the contents of a FarArchive struct.
    /// Can be written to a file to create a .far archive.
    pub fn to_vec(self) -> Vec<u8> {
        // write header
        let mut header = Vec::new();
        for c in "FAR!byAZ".chars() {
            header.push(c as u8);
        }
        header.extend(&self.version.to_le_bytes());
        // wait to write manifest offset until calculated later
        // write file data
        let mut file_data = Vec::new(); // actual data to be written to file
        let mut file_list = Vec::new(); // file list used for making manifest later on
        let mut bytes_written = 16; // where we should start putting files
        for i in 0..self.file_data.len() {
            let mut file_data_bytes = Vec::new();
            file_data_bytes.extend_from_slice(&self.file_data[i].data);
            file_data.extend_from_slice(&file_data_bytes);
            file_list.push(FarFileInfo {
                name: self.file_data[i].name.clone(),
                size: self.file_data[i].size,
                offset: bytes_written,
            });
            bytes_written += self.file_data[i].size;
        }
        // write manifest
        let mut manifest = Vec::new();
        // write file count
        manifest.extend_from_slice(&self.file_count.to_le_bytes());
        // for each file, write (size, size, offset, name length, name)
        for i in 0..self.file_list.len() {
            manifest.extend_from_slice(&file_list[i].size.to_le_bytes());
            manifest.extend_from_slice(&file_list[i].size.to_le_bytes());
            manifest.extend_from_slice(&file_list[i].offset.to_le_bytes());
            manifest.extend_from_slice(&(file_list[i].name.len() as u32).to_le_bytes());
            manifest.extend_from_slice(&file_list[i].name.as_bytes());
        }
        // write manifest offset
        let manifest_offset = bytes_written;
        header.extend_from_slice(&manifest_offset.to_le_bytes());

        // join vecs together
        let mut output = Vec::new();
        output.extend_from_slice(&header);
        output.extend_from_slice(&file_data);
        output.extend_from_slice(&manifest);
        output
    }
}

/// Tests if a buffer is a valid FarArchive.
/// Returns a FarArchive struct if it is, or an error if it is not.
pub fn test(file : &Vec<u8>) -> Result<FarArchive, String> {
    let mut magic = [0u8; 8];
    magic.copy_from_slice(&file[0..8]);
    if magic != *b"FAR!byAZ" {
        return Err("Not a Far archive".to_string());
    }
    let mut version = [0; 4];
    version.copy_from_slice(&file[8..12]);
    let version = u32::from_le_bytes(version);
    // get list of files
    let files = list_files(file).expect("Failed to list files");
    Ok(FarArchive {
        version,
        file_count: files.len() as u32,
        file_list: files,
        file_data: vec![],
    })
}

fn list_files(file : &Vec<u8>) -> Result<Vec<FarFileInfo>, String> {
    let mut read = 12;
    // manifest offset is at 12 bytes (u32)
    let mut offset = [0u8; 4];
    offset.copy_from_slice(&file[read..16]);
    read += 4;
    let offset = u32::from_le_bytes(offset);
    read = offset as usize;
    // move to manifest
    // read u32 for number of files
    let mut num_files = [0u8; 4];
    num_files.copy_from_slice(&file[read..read + 4]);
    read += 4;
    let num_files = u32::from_le_bytes(num_files);
    // for each file, read u32 for size, u32 for size again (stored twice for some reason), u32 for offset, u32 for name length, name
    let mut files = Vec::new();
    for i in 0..num_files {
        let mut size = [0u8; 4];
        size.copy_from_slice(&file[read..read + 4]);
        read += 4;
        let size = u32::from_le_bytes(size);
        let mut size2 = [0u8; 4];
        size2.copy_from_slice(&file[read..read + 4]);
        read += 4;
        let _size2 = u32::from_le_bytes(size2); // why is this stored twice? f*** you EA
        let mut offset = [0u8; 4];
        offset.copy_from_slice(&file[read..read + 4]);
        read += 4;
        let offset = u32::from_le_bytes(offset);
        let mut name_len = [0u8; 4];
        name_len.copy_from_slice(&file[read..read + 4]);
        read += 4;
        let name_len = u32::from_le_bytes(name_len);
        let mut name = vec![0u8; name_len as usize];
        name.copy_from_slice(&file[read..read + name_len as usize]);
        read += name_len as usize;
        files.push(FarFileInfo {
            name: String::from_utf8(name).unwrap(),
            size,
            offset,
        });
    }
    Ok(files)
}