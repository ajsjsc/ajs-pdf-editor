#!/usr/bin/env perl
# AJS fork: rename the product "PdfCraft" to "AJS PDF Editor" in user-visible text.
#
# Only the contents of Rust string literals ("…", b"…", r#"…"#) and the i18n catalogs (*.tsv) are
# changed; comments, identifiers (PdfCraftApp), folder/marker names (PdfCraftData,
# PdfCraftImageCrop) and the attribution to the upstream project stay as they are. Safe to run again
# after merging a new upstream release (idempotent).
#
# Usage (from the repository root): perl tools/ajs/rebrand.pl
#        perl tools/ajs/rebrand.pl --catalogs <craft-i18n checkout> <output dir>   (see below)
use strict;
use warnings;
use File::Find;
use File::Path ();

my $NEW = 'AJS PDF Editor';
my @ROOTS = ('apps/pdfcraft', 'apps/pdfcraft-cli', 'crates');

# "PdfCraft" as a word, not part of an identifier, and not the attribution to the upstream project.
sub rebrand_text {
    my ($s) = @_;
    $s =~ s{(?<![A-Za-z0-9_])PdfCraft(?![A-Za-z0-9_])}{
        my ($pre, $post) = (substr($s, 0, $-[0]), substr($s, $+[0]));
        ($pre =~ /(?:Based on|version of|Fork of)\s*$/ || $post =~ /^\s*(?:source code|by the ArtCraft|contributors)/)
            ? 'PdfCraft' : $NEW
    }ge;
    return $s;
}

# Walk Rust source, rebranding only string literal contents.
sub rebrand_rust {
    my ($src) = @_;
    my $out = '';
    my $n = length $src;
    my $i = 0;
    while ($i < $n) {
        my $rest = substr($src, $i, 2);
        if ($rest eq '//') {                                   # line comment
            my $e = index($src, "\n", $i); $e = $n if $e < 0;
            $out .= substr($src, $i, $e - $i); $i = $e; next;
        }
        if ($rest eq '/*') {                                   # block comment (nested)
            my ($depth, $j) = (1, $i + 2);
            while ($j < $n && $depth) {
                my $t = substr($src, $j, 2);
                if ($t eq '/*') { $depth++; $j += 2 } elsif ($t eq '*/') { $depth--; $j += 2 } else { $j++ }
            }
            $out .= substr($src, $i, $j - $i); $i = $j; next;
        }
        if (substr($src, $i) =~ /\A(b?r(#*)")/) {              # raw string r#"…"#
            my ($open, $hashes) = ($1, $2);
            my $close = '"' . $hashes;
            my $start = $i + length $open;
            my $e = index($src, $close, $start); $e = $n if $e < 0;
            $out .= $open . rebrand_text(substr($src, $start, $e - $start)) . $close;
            $i = $e + length $close; next;
        }
        my $c = substr($src, $i, 1);
        if ($c eq "'") {                                       # char literal or lifetime
            if (substr($src, $i) =~ /\A('(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'\n])')/) {
                $out .= $1; $i += length $1; next;
            }
            $out .= $c; $i++; next;
        }
        if ($c eq '"') {                                       # normal / byte string
            my $j = $i + 1;
            while ($j < $n) {
                my $d = substr($src, $j, 1);
                if ($d eq '\\') { $j += 2; next }
                last if $d eq '"';
                $j++;
            }
            $out .= '"' . rebrand_text(substr($src, $i + 1, $j - $i - 1)) . '"';
            $i = $j + 1; next;
        }
        if ($c =~ /[A-Za-z0-9_]/) {                            # identifier/number: copy whole
            substr($src, $i) =~ /\A([A-Za-z0-9_]+)/;
            $out .= $1; $i += length $1; next;
        }
        $out .= $c; $i++;
    }
    return $out;
}

# Catalog mode: perl tools/ajs/rebrand.pl --catalogs <craft-i18n checkout> <output dir>
#
# Since PdfCraft 0.21 the interface translations live in storytold/craft-i18n, embedded at build time
# from CRAFT_I18N_DIR. The app's English strings say "AJS PDF Editor", so the catalog rows whose
# English text names the product must say it too, or those strings fall back to English. This writes
# <output dir>/locales/<code>/{common,pdfcraft}.tsv: every row whose English column is rebranded gets
# the same rename in its translation (rows keeping the attribution "Based on PdfCraft …" are left
# alone), then tools/ajs/i18n/<code>.tsv (AJS's own strings) is appended to that locale's
# pdfcraft.tsv. Build with CRAFT_I18N_DIR=<output dir>.
sub rebrand_row {
    my ($line) = @_;
    return $line if $line =~ /^\s*(#|$)/;
    my ($eol) = $line =~ /(\r?\n)\z/;
    $eol //= '';
    my $body = substr($line, 0, length($line) - length($eol));
    my @col = split /\t/, $body, -1;
    return $line if @col < 3;
    my $src = rebrand_text($col[1]);
    return $line if $src eq $col[1];
    $col[1] = $src;
    s/(?<![A-Za-z0-9_])PdfCraft(?![A-Za-z0-9_])/$NEW/g for @col[2 .. $#col];
    return join("\t", @col) . $eol;
}

sub slurp { my ($f) = @_; open my $fh, '<:raw', $f or die "$f: $!"; local $/; my $s = <$fh>; close $fh; return $s }
sub spew { my ($f, $s) = @_; open my $fh, '>:raw', $f or die "$f: $!"; print $fh $s; close $fh }

if (@ARGV && $ARGV[0] eq '--catalogs') {
    my (undef, $from, $to) = @ARGV;
    die "usage: perl tools/ajs/rebrand.pl --catalogs <craft-i18n checkout> <output dir>\n" unless $from && $to;
    my $overlays = 'tools/ajs/i18n';
    die "$overlays not found: run from the repository root\n" unless -d $overlays;
    opendir my $dh, "$from/locales" or die "$from is not a craft-i18n checkout ($from/locales: $!)\n";
    my @codes = sort grep { !/^\./ && -f "$from/locales/$_/pdfcraft.tsv" } readdir $dh;
    closedir $dh;
    die "$from/locales has no locale with a pdfcraft.tsv\n" unless @codes;
    my ($rows, $added) = (0, 0);
    for my $code (@codes) {
        my $dir = "$to/locales/$code";
        File::Path::make_path($dir);
        for my $name ('common.tsv', 'pdfcraft.tsv') {
            my $src = "$from/locales/$code/$name";
            next unless -f $src;
            my $out = '';
            for my $line (split /(?<=\n)/, slurp($src)) {
                my $new = rebrand_row($line);
                $rows++ if $new ne $line;
                $out .= $new;
            }
            if ($name eq 'pdfcraft.tsv' && -f "$overlays/$code.tsv") {
                $out .= "\n" unless $out =~ /\n\z/;
                $out .= slurp("$overlays/$code.tsv");
                $added++;
            }
            spew("$to/locales/$code/$name", $out);
        }
    }
    print "rebrand catalogs: ", scalar(@codes), " locales, $rows rows renamed, $added AJS overlays -> $to\n";
    exit 0;
}

my ($files, $changed) = (0, 0);
find({ no_chdir => 1, wanted => sub {
    my $f = $File::Find::name;
    return unless -f $f;
    return unless $f =~ /\.rs$/ || $f =~ m{/i18n/[^/]+\.tsv$};
    return if $f =~ m{(^|/)(target|vendor)/};
    open my $fh, '<:raw', $f or die "$f: $!";
    local $/; my $src = <$fh>; close $fh;
    $files++;
    my $new = $f =~ /\.tsv$/ ? rebrand_text($src) : rebrand_rust($src);
    return if $new eq $src;
    open my $oh, '>:raw', $f or die "$f: $!"; print $oh $new; close $oh;
    $changed++;
}}, @ROOTS);
print "rebrand: $changed of $files files changed\n";
