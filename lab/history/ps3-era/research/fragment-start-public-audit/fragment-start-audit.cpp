#include <array>
#include <cstdint>
#include <cstring>
#include <iostream>
using u32=uint32_t; using u64=uint64_t; using u16=uint16_t; using s32=int32_t;
struct v128 {union {u32 _u32[4];u64 _u64[2];};
static v128 loadu(const void* p,int index) {v128 v;std::memcpy(&v,static_cast<const char*>(p)+index*16,16);return v;}};
namespace rsx::assembler {
enum FP_opcode
	{
		RSX_FP_OPCODE_NOP = 0x00, // No-Operation
		RSX_FP_OPCODE_MOV = 0x01, // Move
		RSX_FP_OPCODE_MUL = 0x02, // Multiply
		RSX_FP_OPCODE_ADD = 0x03, // Add
		RSX_FP_OPCODE_MAD = 0x04, // Multiply-Add
		RSX_FP_OPCODE_DP3 = 0x05, // 3-component Dot Product
		RSX_FP_OPCODE_DP4 = 0x06, // 4-component Dot Product
		RSX_FP_OPCODE_DST = 0x07, // Distance
		RSX_FP_OPCODE_MIN = 0x08, // Minimum
		RSX_FP_OPCODE_MAX = 0x09, // Maximum
		RSX_FP_OPCODE_SLT = 0x0A, // Set-If-LessThan
		RSX_FP_OPCODE_SGE = 0x0B, // Set-If-GreaterEqual
		RSX_FP_OPCODE_SLE = 0x0C, // Set-If-LessEqual
		RSX_FP_OPCODE_SGT = 0x0D, // Set-If-GreaterThan
		RSX_FP_OPCODE_SNE = 0x0E, // Set-If-NotEqual
		RSX_FP_OPCODE_SEQ = 0x0F, // Set-If-Equal
		RSX_FP_OPCODE_FRC = 0x10, // Fraction (fract)
		RSX_FP_OPCODE_FLR = 0x11, // Floor
		RSX_FP_OPCODE_KIL = 0x12, // Kill fragment
		RSX_FP_OPCODE_PK4 = 0x13, // Pack four signed 8-bit values
		RSX_FP_OPCODE_UP4 = 0x14, // Unpack four signed 8-bit values
		RSX_FP_OPCODE_DDX = 0x15, // Partial-derivative in x (Screen space derivative w.r.t. x)
		RSX_FP_OPCODE_DDY = 0x16, // Partial-derivative in y (Screen space derivative w.r.t. y)
		RSX_FP_OPCODE_TEX = 0x17, // Texture lookup
		RSX_FP_OPCODE_TXP = 0x18, // Texture sample with projection (Projective texture lookup)
		RSX_FP_OPCODE_TXD = 0x19, // Texture sample with partial differentiation (Texture lookup with derivatives)
		RSX_FP_OPCODE_RCP = 0x1A, // Reciprocal
		RSX_FP_OPCODE_RSQ = 0x1B, // Reciprocal Square Root
		RSX_FP_OPCODE_EX2 = 0x1C, // Exponentiation base 2
		RSX_FP_OPCODE_LG2 = 0x1D, // Log base 2
		RSX_FP_OPCODE_LIT = 0x1E, // Lighting coefficients
		RSX_FP_OPCODE_LRP = 0x1F, // Linear Interpolation
		RSX_FP_OPCODE_STR = 0x20, // Set-If-True
		RSX_FP_OPCODE_SFL = 0x21, // Set-If-False
		RSX_FP_OPCODE_COS = 0x22, // Cosine
		RSX_FP_OPCODE_SIN = 0x23, // Sine
		RSX_FP_OPCODE_PK2 = 0x24, // Pack two 16-bit floats
		RSX_FP_OPCODE_UP2 = 0x25, // Unpack two 16-bit floats
		RSX_FP_OPCODE_POW = 0x26, // Power
		RSX_FP_OPCODE_PKB = 0x27, // Pack bytes
		RSX_FP_OPCODE_UPB = 0x28, // Unpack bytes
		RSX_FP_OPCODE_PK16 = 0x29, // Pack 16 bits
		RSX_FP_OPCODE_UP16 = 0x2A, // Unpack 16
		RSX_FP_OPCODE_BEM = 0x2B, // Bump-environment map (a.k.a. 2D coordinate transform)
		RSX_FP_OPCODE_PKG = 0x2C, // Pack with sRGB transformation
		RSX_FP_OPCODE_UPG = 0x2D, // Unpack gamma
		RSX_FP_OPCODE_DP2A = 0x2E, // 2-component dot product with scalar addition
		RSX_FP_OPCODE_TXL = 0x2F, // Texture sample with explicit LOD
		RSX_FP_OPCODE_TXB = 0x31, // Texture sample with bias
		RSX_FP_OPCODE_TEXBEM = 0x33,
		RSX_FP_OPCODE_TXPBEM = 0x34,
		RSX_FP_OPCODE_BEMLUM = 0x35,
		RSX_FP_OPCODE_REFL = 0x36, // Reflection vector
		RSX_FP_OPCODE_TIMESWTEX = 0x37,
		RSX_FP_OPCODE_DP2 = 0x38, // 2-component dot product
		RSX_FP_OPCODE_NRM = 0x39, // Normalize
		RSX_FP_OPCODE_DIV = 0x3A, // Division
		RSX_FP_OPCODE_DIVSQ = 0x3B, // Divide by Square Root
		RSX_FP_OPCODE_LIF = 0x3C, // Final part of LIT
		RSX_FP_OPCODE_FENCT = 0x3D, // Fence T?
		RSX_FP_OPCODE_FENCB = 0x3E, // Fence B?
		RSX_FP_OPCODE_BRK = 0x40, // Break
		RSX_FP_OPCODE_CAL = 0x41, // Subroutine call
		RSX_FP_OPCODE_IFE = 0x42, // If
		RSX_FP_OPCODE_LOOP = 0x43, // Loop
		RSX_FP_OPCODE_REP = 0x44, // Repeat
		RSX_FP_OPCODE_RET = 0x45,  // Return


		// Custom opcodes for dependency injection
		RSX_FP_OPCODE_OR16_LO = 0x46,  // Performs a 16-bit OR, taking one register channel as input and overwriting low 16 bits of the output
		RSX_FP_OPCODE_OR16_HI = 0x47,  // Same as the lo variant but now overwrites the high 16-bit block

		// Meta
		RSX_FP_OPCODE_ENUM_MAX = RSX_FP_OPCODE_OR16_HI
	};}
using enum rsx::assembler::FP_opcode;
union OPDEST
{
	u32 HEX;

	struct
	{
		u32 end              : 1; // Set to 1 if this is the last instruction
		u32 dest_reg         : 6; // Destination register index
		u32 fp16             : 1; // Destination is a half register (H0 to H47)
		u32 set_cond         : 1; // Condition Code Registers (CC0 and CC1) are updated
		u32 mask_x           : 1;
		u32 mask_y           : 1;
		u32 mask_z           : 1;
		u32 mask_w           : 1;
		u32 src_attr_reg_num : 4;
		u32 tex_num          : 4;
		u32 exp_tex          : 1; // _bx2
		u32 prec             : 2;
		u32 opcode           : 6;
		u32 no_dest          : 1;
		u32 saturate         : 1; // _sat
	};

	struct
	{
		u32                  : 9;
		u32 write_mask       : 4;
	};

	static OPDEST from_be32(u32 be_word)
	{
		const u32 _hex =
			((be_word & 0x00FF00FF) << 8) |
			((be_word & 0xFF00FF00) >> 8);
		return OPDEST{ .HEX = _hex };
	}
};
namespace program_hash_util {struct fragment_program_utils {
struct fragment_program_metadata
		{
			u32 program_start_offset;
			u32 program_ucode_length;
			u32 program_constants_buffer_length;
			u16 referenced_textures_mask;
			u16 bx2_texture_reads_mask;

			bool has_pack_instructions;
			bool has_branch_instructions;
			bool is_nop_shader;           // Does this affect Z-pass testing???
		};
static bool is_any_src_constant(v128);static fragment_program_metadata analyse_fragment_program(const void*);};}
using program_hash_util::fragment_program_utils;
bool fragment_program_utils::is_any_src_constant(v128 sourceOperand)
{
	const u64 masked = sourceOperand._u64[1] & 0x30000000300;
	return (sourceOperand._u32[1] & 0x300) == 0x200 || (static_cast<u32>(masked) == 0x200 || static_cast<u32>(masked >> 32) == 0x200);
}
fragment_program_utils::fragment_program_metadata fragment_program_utils::analyse_fragment_program(const void* ptr)
{
	fragment_program_utils::fragment_program_metadata result{};
	result.program_start_offset = -1;
	const auto instBuffer = ptr;
	s32 index = 0;

	// Find the start of the program
	while (true)
	{
		const auto inst = v128::loadu(instBuffer, index);

		const u32 opcode = (inst._u32[0] >> 16) & 0x3F;
		if (opcode)
		{
			// We found the start of the program, don't advance the index
			result.program_start_offset = index * 16;
			break;
		}

		if ((inst._u32[0] >> 8) & 0x1)
		{
			result.program_start_offset = index * 16;
			const bool has_literal = is_any_src_constant(inst);
			result.program_ucode_length = has_literal ? 32 : 16;
			result.program_constants_buffer_length = has_literal ? 16 : 0;
			result.is_nop_shader = true;
			return result;
		}

		// NOP source fields can reserve a following literal just like an
		// ordinary instruction. Never interpret that literal as program code.
		index += is_any_src_constant(inst) ? 2 : 1;
	}

	while (true)
	{
		const auto inst = v128::loadu(instBuffer, index);
		const auto d0 = OPDEST::from_be32(inst._u32[0]);
		const auto opcode = static_cast<rsx::assembler::FP_opcode>(d0.opcode);

		switch (opcode)
		{
		case RSX_FP_OPCODE_TEX:
		case RSX_FP_OPCODE_TEXBEM:
		case RSX_FP_OPCODE_TXP:
		case RSX_FP_OPCODE_TXPBEM:
		case RSX_FP_OPCODE_TXD:
		case RSX_FP_OPCODE_TXB:
		case RSX_FP_OPCODE_TXL:
			result.referenced_textures_mask |= (1 << d0.tex_num);
			result.bx2_texture_reads_mask |= ((d0.exp_tex ? 1u : 0u) << d0.tex_num);
			break;
		case RSX_FP_OPCODE_PK4:
		case RSX_FP_OPCODE_UP4:
		case RSX_FP_OPCODE_PK2:
		case RSX_FP_OPCODE_UP2:
		case RSX_FP_OPCODE_PKB:
		case RSX_FP_OPCODE_UPB:
		case RSX_FP_OPCODE_PK16:
		case RSX_FP_OPCODE_UP16:
		case RSX_FP_OPCODE_PKG:
		case RSX_FP_OPCODE_UPG:
			result.has_pack_instructions = true;
			break;
		case RSX_FP_OPCODE_BRK:
		case RSX_FP_OPCODE_CAL:
		case RSX_FP_OPCODE_IFE:
		case RSX_FP_OPCODE_LOOP:
		case RSX_FP_OPCODE_REP:
		case RSX_FP_OPCODE_RET:
			// NOTE: Jump instructions are not yet proved to work outside of loops and if/else blocks
			// Otherwise we would need to follow the execution chain
			result.has_branch_instructions = true;
			break;
		default:
			break;
		}

		if (is_any_src_constant(inst))
		{
			// Instruction references constant, skip one slot occupied by data
			index++;
			result.program_constants_buffer_length += 16;
		}

		index++;

		if ((inst._u32[0] >> 8) & 0x1)
		{
			break;
		}
	}

	result.program_ucode_length = (index - (result.program_start_offset / 16)) * 16;
	return result;
}
int main() {
using Slot=std::array<u32,4>;
auto inst=[](u32 opcode,bool end,int constant=-1){OPDEST d{};d.opcode=opcode;d.end=end;Slot s{OPDEST::from_be32(d.HEX).HEX,0,0,0};if(constant>=0)s[constant+1]=0x200;return s;};
const Slot literal{0x9a99993f,0x6766e63e,0xb9187f3f,0x0000803f};
int total=0,failed=0;
auto test=[&](const char* name,const auto& slots,u32 start,u32 length,u32 constants=0,bool nop=false) {
auto m=fragment_program_utils::analyse_fragment_program(slots.data());++total;
if(m.program_start_offset!=start||m.program_ucode_length!=length||m.program_constants_buffer_length!=constants||m.is_nop_shader!=nop) {
++failed;std::cout<<"FAIL "<<name<<" start="<<m.program_start_offset<<" len="<<m.program_ucode_length<<" const="<<m.program_constants_buffer_length<<" nop="<<m.is_nop_shader<<'\n';}
};
test("ordinary MOV",std::array{inst(1,true)},0,16);
test("plain leading NOP",std::array{inst(0,false),inst(1,true)},16,16);
for(int src=0;src<3;++src) {
test("leading NOP literal",std::array{inst(0,false,src),literal,inst(1,true)},32,16);
test("END NOP literal",std::array{inst(0,true,src),literal},0,32,16,true);
}
test("two NOP literals",std::array{inst(0,false,0),literal,inst(0,false,2),literal,inst(1,true)},64,16);
test("ordinary MOV literal",std::array{inst(1,true,0),literal},0,32,16);
test("END plain NOP",std::array{inst(0,true)},0,16,0,true);
test("skip literal END bit",std::array{inst(0,false,0),literal,inst(2,true)},32,16);
std::cout<<total<<" fixtures, "<<failed<<" failed\n";return failed?1:0;
}