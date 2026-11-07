# Convert the pinned ElanMT tiny model to row-wise signed INT8; vectors stay FP32.
require 'json'
require 'digest'
dir=ARGV.fetch(0)
raw=File.binread(File.join(dir,'elan.safetensors'))
raise 'Wrong model' unless Digest::SHA256.hexdigest(raw)=='e7b1dbc30d6a57db761d92596f60b0234c3b41a2d5191185416b4b25b14cab14'
n=raw.unpack1('Q<'); tensors=JSON.parse(raw[8,n]); base=8+n
halves=Array.new(65536) do |h|
 sign=h&0x8000==0 ? 1.0 : -1.0; e=(h>>10)&31; m=h&1023
 e==0 ? sign*m/(1<<24) : e==31 ? Float::NAN : sign*(1.0+m/1024.0)*2.0**(e-15)
end
names=['model.shared.weight','positions','final_logits_bias']
%w[encoder decoder].each do |side|
 4.times do |layer|
  %w[self_attn encoder_attn].each do |att|
   next if side=='encoder' && att=='encoder_attn'
   %w[q_proj k_proj v_proj out_proj].each { |op| %w[weight bias].each { |suffix| names << "model.#{side}.layers.#{layer}.#{att}.#{op}.#{suffix}" } }
   %w[weight bias].each { |suffix| names << "model.#{side}.layers.#{layer}.#{att}_layer_norm.#{suffix}" }
  end
  %w[fc1 fc2 final_layer_norm].each { |op| %w[weight bias].each { |suffix| names << "model.#{side}.layers.#{layer}.#{op}.#{suffix}" } }
 end
end
raise 'Unexpected layout' unless names.size==171
header=+'BELAN001'.b; header << [names.size].pack('L<'); data=+''.b
names.each do |name|
 if name=='positions'
  rows=512;cols=256;kind=0
  packed=(0...rows).flat_map { |p| (0...128).map { |i|Math.sin(p/10000.0**(2*i/256.0)) }+(0...128).map { |i|Math.cos(p/10000.0**(2*i/256.0)) } }.pack('e*')
 else
  t=tensors.fetch(name);raise 'Unexpected tensor type' unless t.fetch('dtype')=='F16'
  shape=t.fetch('shape');first,last=t.fetch('data_offsets');values=raw[base+first,last-first].unpack('S<*').map{|h|halves[h]}
  raise 'Nonfinite weight' unless values.all?(&:finite?)
  rows,cols=shape.size==2 ? shape : [1,shape[0]]
  kind=shape.size==2 && name!='final_logits_bias' ? 1 : 0
  packed=if kind==1
   values.each_slice(cols).map do |row|
    max=row.map(&:abs).max;scale=max==0 ? 1.0 : max/127.0
    [scale].pack('e')+row.map{|x|(x/scale).round.clamp(-127,127)}.pack('c*')
   end.join.b
  else; values.pack('e*');end
 end
 header << [12+171*16+data.bytesize,kind,rows,cols].pack('L<*');data<<packed
end
model=header+data
File.binwrite(File.join(dir,'elan.bin'),model)
File.binwrite(File.join(dir,'elan.bin.sha256'),Digest::SHA256.digest(model))
# SentencePiece ModelProto: extract normal pieces and unigram scores. ASCII input uses nmt_nfkc's whitespace rules.
def fields(s)
 out=[];i=0;var=-> {v=0;shift=0;loop{b=s.getbyte(i);raise 'Truncated protobuf' unless b;i+=1;v|=(b&127)<<shift;break if b<128;shift+=7;raise 'Oversized varint' if shift>63};v}
 while i<s.bytesize
  tag=var.call; v=case tag&7
  when 0;var.call
  when 1;r=s[i,8];i+=8;r
  when 2;n=var.call;r=s[i,n];i+=n;r
  when 5;r=s[i,4];i+=4;r
  else;raise 'Unsupported protobuf wire type';end
  raise 'Truncated protobuf' if i>s.bytesize
  out << [tag>>3,v]
 end;out
end
sp=File.binread(File.join(dir,'elan-source.spm'))
raise 'Wrong tokenizer' unless Digest::SHA256.hexdigest(sp)=='a6824c2996a4877cae8a0240f11a112ab0e4ea1f22e54cd42179376fe558f4ff'
pieces=fields(sp).select{|k,_| k==1}.map{|_,v|fields(v).to_h}
raise 'Wrong vocabulary' unless pieces.size==32001
vocab=JSON.parse(File.read(File.join(dir,'elan-vocab.json')))
pieces.each_with_index{|p,i|raise 'Vocabulary mismatch' unless vocab.fetch(p.fetch(1).dup.force_encoding('UTF-8'))==i}
ids=pieces.each_index.select{|i| !pieces[i].key?(3) || pieces[i][3]==1}.sort_by{|i|pieces[i].fetch(1)}
strings=pieces.map{|p|p.fetch(1)}; offsets=[0];strings.each{|s|offsets<<offsets.last+s.bytesize}
tok='BELANT01'.b+[ids.size].pack('L<')+offsets.pack('L<*')+pieces.map{|p|p.fetch(2)}.join+ids.pack('L<*')+strings.join
File.binwrite(File.join(dir,'elan.tok'),tok)
File.binwrite(File.join(dir,'elan.tok.sha256'),Digest::SHA256.digest(tok))
puts "Elan INT8 #{model.bytesize} bytes; tokenizer #{tok.bytesize} bytes"
