
#include <iostream>
#include <unordered_map>
#include <vector>
#define ASSERT_AND_CONTINUE(x) if (!(x)) continue
namespace spirv { struct ResourceInterfaceVariable { unsigned module, set; const char* type; }; }
struct EntryPoint { std::unordered_map<unsigned, const spirv::ResourceInterfaceVariable*> resource_interface_variable_map; };
struct ShaderStageState { const EntryPoint* entrypoint; };
struct Pipeline { std::vector<ShaderStageState> stage_states; };
struct Access { unsigned variable_id; };
const spirv::ResourceInterfaceVariable* lookup(const Pipeline* pipeline_state, Access descriptor_access) {
    const spirv::ResourceInterfaceVariable* resource_variable = nullptr;
{
                        for (const ShaderStageState& stage_state : pipeline_state->stage_states) {
                            ASSERT_AND_CONTINUE(stage_state.entrypoint);
                            auto variable_it =
                                stage_state.entrypoint->resource_interface_variable_map.find(descriptor_access.variable_id);
                            if (variable_it != stage_state.entrypoint->resource_interface_variable_map.end()) {
                                resource_variable = variable_it->second;
                                break;  // Only need to find a single entry point
                            }
                        }
                    }
    return resource_variable;
}
int main() {
    spirv::ResourceInterfaceVariable vertex_buffer{1,0,"unsigned texel buffer"}, fragment_image{2,1,"float image"};
    EntryPoint vertex{{{17,&vertex_buffer}}}, fragment{{{17,&fragment_image}}};
    Pipeline stages{{{&vertex},{&fragment}}};
    int checks=0, failures=0;
    auto check=[&](const char* name,bool ok) { ++checks; if(!ok) { ++failures; std::cout<<"FAIL "<<name<<'\n'; } };
    // A fragment access has the same module-local variable ID as the vertex buffer.
    check("fragment access can receive vertex resource metadata", lookup(&stages,{17})==&vertex_buffer);
    stages.stage_states={{&fragment},{&vertex}};
    check("reversed stages can misassociate a vertex access", lookup(&stages,{17})==&fragment_image);
    fragment.resource_interface_variable_map={{18,&fragment_image}};
    stages.stage_states={{&vertex},{&fragment}};
    check("distinct module-local IDs avoid this counterexample", lookup(&stages,{18})==&fragment_image);
    check("unmatched ID returns no resource", lookup(&stages,{99})==nullptr);
    std::cout<<checks<<" stage-lookup checks, "<<failures<<" failed; "
             <<"two cross-stage counterexamples reproduced (no native causality claim)\n";
    return failures ? 1 : 0;
}
