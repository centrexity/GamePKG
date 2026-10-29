// GamePKG bounded saved-file protocol, Zlib license; see ../upstream/readme_source.txt.
#include <dirent.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <string>
#include <vector>
#include <algorithm>
static const size_t gamepkg_tree_limit = 32*1048576, gamepkg_file_limit = 16*1048576;
struct gamepkg_saved_file
{
    std::string name;
    std::vector<unsigned char> bytes;
};
static bool gamepkg_save_path(const std::string &name)
{
    if(name.empty() || name.size() > 240) return false;
    unsigned parts = 1; size_t start = 0;
    for(size_t i=0;i<=name.size();i++)
    {
        if(i==name.size() || name[i]=='/')
        {
            if(i==start || name[start]=='.' || ++parts > 7) return false;
            start = i+1;
        }
        else if(!((name[i]>='a' && name[i]<='z') || (name[i]>='A' && name[i]<='Z') ||
                  (name[i]>='0' && name[i]<='9') || name[i]=='.' || name[i]=='_' || name[i]=='-')) return false;
    }
    if(name=="config.cfg" || name=="init.cfg" || name=="servers.cfg" || name=="auth.cfg" || name=="autoexec.cfg") return true;
    const char *extensions[] = { ".ogz", ".cfg", ".BAK", ".jpg", ".png", ".dmo", ".avi" };
    for(unsigned i=0;i<7;i++)
    {
        size_t length = strlen(extensions[i]);
        if(name.size() < length || (i>=5 ? strcasecmp(name.c_str()+name.size()-length, extensions[i]) :
            name.compare(name.size()-length, length, extensions[i]))) continue;
        return i>=5 ? true :
            name.compare(0, 9, "packages/")==0 && name.find('/', 9)!=std::string::npos;
    }
    return false;
}
static void gamepkg_save_failure()
{
    fprintf(stderr, "Cube 2 saved files exceeded their quota or failed validation. The previous save was preserved.\n");
    exit(2);
}
static bool gamepkg_file_less(const gamepkg_saved_file &a, const gamepkg_saved_file &b) { return a.name < b.name; }
static void gamepkg_collect(int fd, const std::string &prefix, unsigned depth,
    std::vector<gamepkg_saved_file> &files, size_t &total, unsigned &visited)
{
    int copy = openat(fd, ".", O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);
    if(copy < 0) gamepkg_save_failure();
    DIR *directory = fdopendir(copy);
    if(!directory) { close(copy); gamepkg_save_failure(); }
    for(;;)
    {
        errno = 0;
        struct dirent *entry = readdir(directory);
        if(!entry) { if(errno) gamepkg_save_failure(); break; }
        if(!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, "..")) continue;
        if(++visited > 2048 || depth > 6) gamepkg_save_failure();
        const std::string name = prefix+entry->d_name;
        struct stat st;
        if(fstatat(fd, entry->d_name, &st, AT_SYMLINK_NOFOLLOW)) gamepkg_save_failure();
        if(S_ISDIR(st.st_mode))
        {
            if(depth>=6 || name.size()>240) gamepkg_save_failure();
            int child = openat(fd, entry->d_name, O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);
            if(child<0) gamepkg_save_failure();
            gamepkg_collect(child, name+"/", depth+1, files, total, visited);
            close(child); continue;
        }
        if(!gamepkg_save_path(name)) continue;
        int file = openat(fd, entry->d_name, O_RDONLY|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC);
        if(file<0 || fstat(file, &st) || !S_ISREG(st.st_mode) || st.st_nlink!=1 ||
           st.st_size<0 || uint64_t(st.st_size)>gamepkg_file_limit || files.size()>=256 ||
           total+16+name.size()+uint64_t(st.st_size)>gamepkg_tree_limit) gamepkg_save_failure();
        gamepkg_saved_file saved; saved.name = name; saved.bytes.resize(size_t(st.st_size));
        if(!gamepkg_transfer(file, saved.bytes.data(), saved.bytes.size(), false)) gamepkg_save_failure();
        unsigned char extra;
        if(read(file, &extra, 1)!=0) gamepkg_save_failure();
        close(file);
        total += 16+name.size()+saved.bytes.size();
        files.push_back(saved);
    }
    closedir(directory);
}
static void gamepkg_restore_tree(const unsigned char *header)
{
    const uint32_t count = gamepkg_le32(header+4), size = gamepkg_le32(header+8);
    if(count>256 || size>gamepkg_tree_limit-16 || gamepkg_le32(header+12)) gamepkg_save_failure();
    std::vector<unsigned char> payload(size);
    if(!gamepkg_transfer(0, payload.data(), size, false)) gamepkg_save_failure();
    std::vector<gamepkg_saved_file> files;
    size_t offset = 0; std::string previous;
    for(unsigned i=0;i<count;i++)
    {
        if(size-offset<16) gamepkg_save_failure();
        const unsigned char *record = payload.data()+offset;
        const uint32_t length = gamepkg_le32(record), bytes = gamepkg_le32(record+4);
        if(!length || length>240 || bytes>gamepkg_file_limit || gamepkg_le32(record+8) || gamepkg_le32(record+12)) gamepkg_save_failure();
        offset += 16;
        if(length>size-offset) gamepkg_save_failure();
        gamepkg_saved_file saved;
        saved.name.assign(reinterpret_cast<const char *>(payload.data()+offset), length);
        if(!gamepkg_save_path(saved.name) || saved.name<=previous) gamepkg_save_failure();
        for(size_t j=0;j<files.size();j++)
            if(saved.name.compare(0, files[j].name.size()+1, files[j].name+"/")==0) gamepkg_save_failure();
        previous = saved.name; offset += length;
        if(bytes>size-offset) gamepkg_save_failure();
        saved.bytes.assign(payload.begin()+offset, payload.begin()+offset+bytes);
        offset += bytes; files.push_back(saved);
    }
    if(offset!=size) gamepkg_save_failure();
    int root = open("/save", O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);
    if(root<0) gamepkg_save_failure();
    for(size_t i=0;i<files.size();i++)
    {
        int parent = openat(root, ".", O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);
        if(parent<0) gamepkg_save_failure();
        size_t start = 0, slash;
        while((slash=files[i].name.find('/', start))!=std::string::npos)
        {
            const std::string part = files[i].name.substr(start, slash-start);
            if(mkdirat(parent, part.c_str(), 0700) && errno!=EEXIST) gamepkg_save_failure();
            int child = openat(parent, part.c_str(), O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);
            if(child<0) gamepkg_save_failure();
            close(parent); parent = child; start = slash+1;
        }
        int file = openat(parent, files[i].name.c_str()+start, O_WRONLY|O_CREAT|O_EXCL|O_NOFOLLOW|O_CLOEXEC, 0600);
        if(file<0 || !gamepkg_transfer(file, files[i].bytes.data(), files[i].bytes.size(), true)) gamepkg_save_failure();
        close(file); close(parent);
    }
    close(root);
}
static void gamepkg_tree_snapshot(int output)
{
    int root = open("/save", O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC);
    if(root<0) gamepkg_save_failure();
    std::vector<gamepkg_saved_file> files;
    size_t total = 16; unsigned visited = 0;
    gamepkg_collect(root, "", 1, files, total, visited); close(root);
    // Cube's global swap template conflicts with std::sort's ADL lookup.
    // The bounded set needs at most 32640 explicit moves.
    for(size_t i=1;i<files.size();i++)
        for(size_t j=i;j>0 && gamepkg_file_less(files[j], files[j-1]);j--)
            std::swap(files[j], files[j-1]);
    unsigned char header[16]; memset(header, 0, sizeof(header)); memcpy(header, "GPS2", 4);
    gamepkg_put32(header+4, files.size()); gamepkg_put32(header+8, total-16);
    if(!gamepkg_transfer(output, header, sizeof(header), true)) exit(0);
    for(size_t i=0;i<files.size();i++)
    {
        unsigned char record[16]; memset(record, 0, sizeof(record));
        gamepkg_put32(record, files[i].name.size()); gamepkg_put32(record+4, files[i].bytes.size());
        if(!gamepkg_transfer(output, record, sizeof(record), true) ||
           !gamepkg_transfer(output, const_cast<char *>(files[i].name.data()), files[i].name.size(), true) ||
           !gamepkg_transfer(output, files[i].bytes.data(), files[i].bytes.size(), true)) exit(0);
    }
}
