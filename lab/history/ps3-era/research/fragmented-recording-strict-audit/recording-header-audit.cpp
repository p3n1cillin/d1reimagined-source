
#include <iostream>
#include <map>
#include <string>
#include <string_view>
using AVDictionary = std::map<std::string, std::string>;
struct Format { const char* name; };
struct Context { const Format* oformat; };
int set_calls=0, free_calls=0, header_calls=0, fail_at=0, header_result=0;
AVDictionary captured;
int av_dict_set(AVDictionary** d,const char* key,const char* value,int) {
    if (++set_calls == fail_at) return -12;
    if (!*d) *d=new AVDictionary;
    (**d)[key]=value; return 0;
}
void av_dict_free(AVDictionary** d) { ++free_calls; delete *d; *d=nullptr; }
int avformat_write_header(Context*,AVDictionary** d) {
    ++header_calls; captured=d && *d ? **d : AVDictionary{}; return header_result;
}
int configure(const char* name,int failure=0,int header_failure=0) {
    set_calls=free_calls=header_calls=0; captured.clear();
    fail_at=failure; header_result=header_failure;
    Format format{name}; Context context{&format};
    struct { Context* format_context; } av{&context};
AVDictionary* mux_options = nullptr;
			int header_error = 0;
			const std::string_view mux_name = av.format_context->oformat->name;
			if (mux_name == "mov" || mux_name == "mp4")
			{
				header_error = av_dict_set(&mux_options, "movflags", "frag_keyframe+empty_moov+default_base_moof", 0);
				if (header_error >= 0)
				{
					header_error = av_dict_set(&mux_options, "frag_duration", "1000000", 0);
				}
			}
			if (header_error >= 0)
			{
				header_error = avformat_write_header(av.format_context, &mux_options);
			}
			av_dict_free(&mux_options);

			
    return header_error;
}
int checks=0, failures=0;
void check(const char* label,bool valid) { ++checks; if (!valid) { ++failures; std::cerr<<"FAIL: "<<label<<"\n"; } }
int main() {
    for (auto name : {"mov","mp4"}) {
        check("MOV/MP4 header succeeds", configure(name)==0 && header_calls==1);
        check("interrupted container has initial metadata and bounded fragments",
            captured["movflags"]=="frag_keyframe+empty_moov+default_base_moof" && captured["frag_duration"]=="1000000");
        check("options are freed after header", free_calls==1);
    }
    for (auto name : {"matroska","avi","wav"}) {
        check("other muxer keeps default header", configure(name)==0 && header_calls==1 && captured.empty() && set_calls==0);
    }
    check("first option allocation failure propagates without writing header", configure("mov",1)==-12 && header_calls==0 && free_calls==1);
    check("second option allocation failure releases first option", configure("mp4",2)==-12 && header_calls==0 && free_calls==1);
    check("header failure propagates and releases options", configure("mov",0,-22)==-22 && header_calls==1 && free_calls==1);
    std::cout<<checks<<" recording-header checks, "<<failures<<" failed\n";
    return failures ? 1 : 0;
}
